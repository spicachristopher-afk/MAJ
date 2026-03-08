@echo off
setlocal

:: ─────────────────────────────────────────────────────────
:: PC Updater — Script de lancement
:: Requiert les privilèges Administrateur (pour les mises à
:: jour système et l'installation de modules PowerShell).
:: ─────────────────────────────────────────────────────────

:: Vérification des privilèges Administrateur
>nul 2>&1 "%SYSTEMROOT%\system32\cacls.exe" "%SYSTEMROOT%\system32\config\system"
if '%errorlevel%' NEQ '0' (
    echo Demande de privileges administrateur...
    goto UACPrompt
) else (
    goto gotAdmin
)

:UACPrompt
    echo Set UAC = CreateObject^("Shell.Application"^) > "%temp%\getadmin.vbs"
    echo UAC.ShellExecute "%~s0", "", "", "runas", 1 >> "%temp%\getadmin.vbs"
    "%temp%\getadmin.vbs"
    del "%temp%\getadmin.vbs"
    exit /B

:gotAdmin
    pushd "%CD%"
    CD /D "%~dp0"

    echo ===================================
    echo  PC Updater — Verification
    echo ===================================
    echo.

    :: Vérification des dépendances Node.js
    if not exist "node_modules" (
        echo [INFO] node_modules manquant — execution de npm install...
        call npm install
        if errorlevel 1 (
            echo [ERREUR] npm install a échoue. Vérifiez votre installation Node.js.
            pause
            exit /B 1
        )
        echo.
    )

    :: Vérification de Rust / Cargo
    where cargo >nul 2>&1
    if errorlevel 1 (
        echo [ERREUR] Cargo ^(Rust^) n'est pas installe ou n'est pas dans le PATH.
        echo Installez Rust depuis https://rustup.rs
        pause
        exit /B 1
    )

    echo [OK] Dependances verifiees.
    echo.
    echo Lancement de PC Updater en mode developpement...
    echo.
    call npm run tauri dev
    pause

endlocal