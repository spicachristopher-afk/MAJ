/**
 * PC Updater — Frontend Logic
 * Architecture modulaire : State → UI → Events
 */

const { invoke } = window.__TAURI__.tauri;
const { listen } = window.__TAURI__.event;

// ─────────────────────────────────────────────
// Configuration des gestionnaires
// ─────────────────────────────────────────────
const MANAGERS = {
  "Winget":         { id: "winget",    color: "#0099ff" },
  "Scoop":          { id: "scoop",     color: "#f5d020" },
  "Chocolatey":     { id: "choco",     color: "#c28c3b" },
  "Windows Update": { id: "winupdate", color: "#0078d4" },
  "Système":        { id: "system",    color: "#00e676" },
};

const STATUS_ICONS = {
  pending: "⏳",
  running: "⚙️",
  success: "✅",
  error:   "❌",
};

const STATUS_LABELS = {
  pending: "En attente",
  running: "En cours...",
  success: "Terminé",
  error:   "Erreur",
};

// ─────────────────────────────────────────────
// State
// ─────────────────────────────────────────────
const state = {
  isRunning: false,
  managerStatus: Object.fromEntries(
    Object.keys(MANAGERS).map(k => [k, "pending"])
  ),
  counts: { done: 0, total: Object.keys(MANAGERS).filter(m => m !== "Système").length },
};

// ─────────────────────────────────────────────
// DOM refs (initialisées après DOMContentLoaded)
// ─────────────────────────────────────────────
let dom = {};

// ─────────────────────────────────────────────
// UI Helpers
// ─────────────────────────────────────────────

function escapeHtml(str) {
  return str
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#039;");
}

function setManagerStatus(managerName, status) {
  const mgr = MANAGERS[managerName];
  if (!mgr) return;

  state.managerStatus[managerName] = status;

  const card       = document.getElementById(`card-${mgr.id}`);
  const iconEl     = document.getElementById(`icon-${mgr.id}`);
  const statusEl   = document.getElementById(`status-${mgr.id}`);
  if (!card || !iconEl || !statusEl) return;

  // Reset classes
  card.classList.remove("pending", "running", "success", "error");
  card.classList.add(status);

  iconEl.textContent   = STATUS_ICONS[status];
  statusEl.textContent = STATUS_LABELS[status];

  if (status === "running") {
    iconEl.innerHTML = `<span class="spinning">${STATUS_ICONS.running}</span>`;
  }
}

function setGlobalStatus(label, mode = "") {
  const badge = document.getElementById("global-status");
  const text  = document.getElementById("global-status-text");
  badge.className = `status-badge ${mode}`;
  text.textContent = label;
}

function updateProgress() {
  const done  = Object.entries(state.managerStatus)
    .filter(([k]) => k !== "Système")
    .filter(([, v]) => v === "success" || v === "error")
    .length;
  const total = state.counts.total;
  const pct   = total > 0 ? Math.round((done / total) * 100) : 0;
  dom.progressBar.style.width = `${pct}%`;
}

function appendLog(managerName, message, classes = []) {
  const mgr    = MANAGERS[managerName] ?? { color: "#8892a4" };
  const isErr  = message.startsWith("ERROR:");
  const isHead = message.startsWith("---") || message.startsWith("===");

  const line = document.createElement("div");
  line.className = ["log-line", ...classes].join(" ");
  if (isErr)  line.classList.add("is-error");
  if (isHead) line.classList.add("is-header");

  line.innerHTML = `
    <span class="log-tag" style="color:${mgr.color}">[${escapeHtml(managerName)}]</span>
    <span class="log-msg">${escapeHtml(message)}</span>
  `;

  dom.terminal.appendChild(line);
  dom.terminal.scrollTop = dom.terminal.scrollHeight;
}

function resetState() {
  state.isRunning = true;

  // Reset all manager statuses
  Object.keys(MANAGERS).forEach(m => setManagerStatus(m, "pending"));

  // Clear terminal
  dom.terminal.innerHTML = "";

  // Reset progress bar
  dom.progressBar.style.width = "0%";

  // Button
  dom.btnUpdate.disabled    = true;
  dom.btnUpdate.textContent = "Mise à jour en cours...";

  setGlobalStatus("En cours...", "running");
}

function finalizeState() {
  state.isRunning = false;

  dom.btnUpdate.disabled    = false;
  dom.btnUpdate.textContent = "Relancer les mises à jour";
  dom.progressBar.style.width = "100%";

  const hasError = Object.entries(state.managerStatus)
    .filter(([k]) => k !== "Système")
    .some(([, v]) => v === "error");

  setGlobalStatus(hasError ? "Terminé avec erreurs" : "Terminé ✓", "done");
}

// ─────────────────────────────────────────────
// Event Handlers
// ─────────────────────────────────────────────

function handleLogEvent(event) {
  const { manager, message } = event.payload;
  if (!message || !message.trim()) return;

  // Détecter start/end d'un gestionnaire via les marqueurs du backend
  if (message.startsWith("--- DÉMARRAGE")) {
    setManagerStatus(manager, "running");
  } else if (message.startsWith("--- SUCCÈS")) {
    setManagerStatus(manager, "success");
    updateProgress();
  } else if (message.startsWith("--- ERREUR")) {
    setManagerStatus(manager, "error");
    updateProgress();
  } else if (message.includes("TOUTES LES OPÉRATIONS SONT TERMINÉES")) {
    finalizeState();
  }

  appendLog(manager, message);
}

async function startUpdates() {
  resetState();

  try {
    await invoke("run_all_updates");
  } catch (err) {
    appendLog("Système", `ERROR: ${err}`, ["is-error"]);
    dom.btnUpdate.disabled    = false;
    dom.btnUpdate.textContent = "Réessayer";
    setGlobalStatus("Erreur critique", "");
    state.isRunning = false;
  }
}

// ─────────────────────────────────────────────
// Init
// ─────────────────────────────────────────────
window.addEventListener("DOMContentLoaded", async () => {
  dom = {
    terminal:    document.getElementById("terminal"),
    btnUpdate:   document.getElementById("btn-update"),
    btnClear:    document.getElementById("btn-clear"),
    progressBar: document.getElementById("progress-bar"),
  };

  // Écouter les événements de logs du backend
  await listen("update-log", handleLogEvent);

  dom.btnUpdate.addEventListener("click", () => {
    if (!state.isRunning) startUpdates();
  });

  dom.btnClear.addEventListener("click", () => {
    if (!state.isRunning) {
      dom.terminal.innerHTML =
        '<div class="terminal-placeholder">Terminal effacé.</div>';
    }
  });
});