// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::io::{BufRead, BufReader};
use std::os::windows::process::CommandExt;
use std::process::{Command, Stdio};
use std::thread;
use tauri::Window;

// ─── Constantes ───────────────────────────────────────────────────────────────

const CREATE_NO_WINDOW: u32 = 0x08000000;

const MANAGER_WINGET:     &str = "Winget";
const MANAGER_SCOOP:      &str = "Scoop";
const MANAGER_CHOCOLATEY: &str = "Chocolatey";
const MANAGER_WINUPDATE:  &str = "Windows Update";
const MANAGER_SYSTEM:     &str = "Système";

// ─── Types ────────────────────────────────────────────────────────────────────

#[derive(Clone, serde::Serialize)]
struct LogPayload {
    manager: String,
    message: String,
}

// ─── Utilitaires ──────────────────────────────────────────────────────────────

/// Envoie un message de log vers le frontend.
fn emit_log(window: &Window, manager: &str, message: impl Into<String>) {
    let _ = window.emit(
        "update-log",
        LogPayload {
            manager: manager.to_string(),
            message: message.into(),
        },
    );
}

/// Écrit le script PS dans un fichier .ps1 temporaire et l'exécute via `-File`.
/// Cette approche évite tous les problèmes de quoting/escaping liés à `-Command`.
/// Le fichier temp est supprimé après exécution.
fn run_powershell(window: &Window, manager: &str, ps_script: &str) {
    emit_log(window, manager, format!("--- DÉMARRAGE : {} ---", manager));

    // Chemin du fichier temp
    let tmp_dir  = std::env::var("TEMP").unwrap_or_else(|_| "C:\\Windows\\Temp".to_string());
    let tmp_name = manager.replace(' ', "_").replace(':', "");
    let tmp_path = format!("{}\\pc_updater_{}.ps1", tmp_dir, tmp_name);

    // Écriture du script
    if let Err(e) = std::fs::write(&tmp_path, ps_script) {
        emit_log(window, manager, format!("ERROR: Impossible d'écrire le script temp: {}", e));
        emit_log(window, manager, format!("--- ERREUR/FIN AVEC CODE -1 : {} ---", manager));
        return;
    }

    let child_result = Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy", "Bypass",
            "-File", &tmp_path,
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn();

    let mut child = match child_result {
        Ok(c) => c,
        Err(e) => {
            emit_log(window, manager, format!("ERROR: Impossible de lancer PowerShell: {}", e));
            emit_log(window, manager, format!("--- ERREUR/FIN AVEC CODE -1 : {} ---", manager));
            let _ = std::fs::remove_file(&tmp_path);
            return;
        }
    };

    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();

    // Thread stdout
    let win_out = window.clone();
    let mgr_out = manager.to_string();
    let stdout_thread = thread::spawn(move || {
        for line in BufReader::new(stdout).lines().flatten() {
            emit_log(&win_out, &mgr_out, line);
        }
    });

    // Thread stderr
    let win_err = window.clone();
    let mgr_err = manager.to_string();
    let stderr_thread = thread::spawn(move || {
        for line in BufReader::new(stderr).lines().flatten() {
            emit_log(&win_err, &mgr_err, format!("ERROR: {}", line));
        }
    });

    let status = child.wait();
    let _ = stdout_thread.join();
    let _ = stderr_thread.join();
    let _ = std::fs::remove_file(&tmp_path); // Nettoyage du fichier temp

    let end_msg = match status {
        Ok(s) if s.success() => format!("--- SUCCÈS : {} ---", manager),
        Ok(s) => format!(
            "--- ERREUR/FIN AVEC CODE {} : {} ---",
            s.code().unwrap_or(-1),
            manager
        ),
        Err(e) => format!("--- ERREUR/FIN (attente échouée) : {} — {} ---", manager, e),
    };

    emit_log(window, manager, end_msg);
}

// ─── Scripts PowerShell ───────────────────────────────────────────────────────

fn script_scoop(shims: &str) -> String {
    // Vérifie scoop.ps1 ET scoop.cmd pour compatibilité toutes versions de Scoop.
    // Le chemin est injecté depuis Rust (résolu via std::env::var), pas d'expansion PS requise.
    format!(r#"
$scoopShims = '{shims}'
$scoopCmd = Get-Command scoop -ErrorAction SilentlyContinue

if (-not $scoopCmd) {{
    if (Test-Path "$scoopShims\scoop.cmd") {{
        $env:PATH = "$scoopShims;$env:PATH"
    }} elseif (Test-Path "$scoopShims\scoop.ps1") {{
        $env:PATH = "$scoopShims;$env:PATH"
    }} else {{
        Write-Output "[INFO] Scoop n'est pas installe sur ce systeme - ignore."
        exit 0
    }}
}}

scoop update
scoop update *
"#, shims = shims)
}

fn script_winget() -> &'static str {
    "winget upgrade --all --accept-package-agreements --accept-source-agreements"
}

fn script_choco() -> &'static str {
    "choco upgrade all -y"
}

fn script_windows_update() -> &'static str {
    r#"
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12

$isAdmin = ([Security.Principal.WindowsPrincipal] `
            [Security.Principal.WindowsIdentity]::GetCurrent() `
           ).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)

if (-not $isAdmin) {
    Write-Output "[INFO] Privileges administrateur requis - relancement en mode eleve..."

    $tmpLog = [System.IO.Path]::GetTempFileName()
    $tmpPs1 = $tmpLog + ".ps1"

    $innerScript = @'
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
if (-not (Get-Module -ListAvailable -Name PSWindowsUpdate)) {
    Install-Module -Name PSWindowsUpdate -Force -SkipPublisherCheck -Scope CurrentUser
}
Import-Module PSWindowsUpdate -Force
Get-WindowsUpdate -AcceptAll -Install -IgnoreReboot
'@
    Set-Content -Path $tmpPs1 -Value $innerScript -Encoding UTF8

    Start-Process powershell.exe -Verb RunAs -Wait -WindowStyle Hidden `
        -ArgumentList "-NoProfile -NonInteractive -ExecutionPolicy Bypass -File `"$tmpPs1`" *> `"$tmpLog`""

    if (Test-Path $tmpLog) {
        Get-Content $tmpLog
        Remove-Item $tmpLog -Force -ErrorAction SilentlyContinue
    }
    if (Test-Path $tmpPs1) {
        Remove-Item $tmpPs1 -Force -ErrorAction SilentlyContinue
    }
} else {
    if (-not (Get-Module -ListAvailable -Name PSWindowsUpdate)) {
        Install-Module -Name PSWindowsUpdate -Force -SkipPublisherCheck -Scope CurrentUser
    }
    Import-Module PSWindowsUpdate -Force
    Get-WindowsUpdate -AcceptAll -Install -IgnoreReboot
}
"#
}

// ─── Commandes Tauri ──────────────────────────────────────────────────────────

#[tauri::command]
async fn run_all_updates(window: Window) -> Result<(), String> {
    let mut handles: Vec<thread::JoinHandle<()>> = Vec::new();

    // 1. Scoop — chemin résolu en Rust pour éviter tout escaping PowerShell
    {
        let win = window.clone();
        let userprofile = std::env::var("USERPROFILE").unwrap_or_default();
        let scoop_base  = std::env::var("SCOOP")
            .unwrap_or_else(|_| format!("{}\\scoop", userprofile));
        let shims  = format!("{}\\shims", scoop_base);
        let script = script_scoop(&shims);
        handles.push(thread::spawn(move || {
            run_powershell(&win, MANAGER_SCOOP, &script);
        }));
    }

    // 2. Winget
    {
        let win = window.clone();
        handles.push(thread::spawn(move || {
            run_powershell(&win, MANAGER_WINGET, script_winget());
        }));
    }

    // 3. Chocolatey
    {
        let win = window.clone();
        handles.push(thread::spawn(move || {
            run_powershell(&win, MANAGER_CHOCOLATEY, script_choco());
        }));
    }

    // 4. Windows Update — auto-élévation si nécessaire
    {
        let win = window.clone();
        handles.push(thread::spawn(move || {
            run_powershell(&win, MANAGER_WINUPDATE, script_windows_update());
        }));
    }

    // Attendre la fin de tous les threads
    for handle in handles {
        let _ = handle.join();
    }

    emit_log(
        &window,
        MANAGER_SYSTEM,
        "=== TOUTES LES OPÉRATIONS SONT TERMINÉES ===",
    );

    Ok(())
}

// ─── Point d'entrée ───────────────────────────────────────────────────────────

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![run_all_updates])
        .run(tauri::generate_context!())
        .expect("Erreur lors de l'exécution de l'application Tauri");
}