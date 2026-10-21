/*
 * Preload de diagnostic du mode « compat ».
 *
 * Electron 45 a retiré le module `remote` ; le renderer d'Obsidian 1.14.4
 * l'utilise. Ce preload (notre code) installe une surface `remote` minimale
 * sur le vrai module `electron`, et journalise les erreurs de la page, les
 * rejets de promesses non gérés et les erreurs non capturées du processus
 * renderer — sans quoi un échec de boot est invisible (la fenêtre se ferme
 * et le journal reste muet).
 */
const electron = require('electron');

/* Journalisation côté renderer : tout arrive sur stderr du processus principal. */
function journaliser(prefixe, detail) {
  process.stderr.write('[renderer] ' + prefixe + ' ' + detail + '\n');
}

window.addEventListener('error', (evenement) => {
  journaliser(
    'erreur page :',
    (evenement.message || '') + ' @ ' + (evenement.filename || '?') + ':' + (evenement.lineno || '?')
  );
});

window.addEventListener('unhandledrejection', (evenement) => {
  const raison = evenement.reason;
  journaliser('rejet non géré :', (raison && raison.stack) || String(raison));
});

process.on('uncaughtException', (erreur) => {
  journaliser('exception non capturée :', (erreur && erreur.stack) || String(erreur));
});

/* Surface `remote` minimale : la fenêtre courante et ses états. */
const fenetreCourante = {
  minimizable: () => true,
  maximizable: () => true,
  closable: () => true,
  isFullScreen: () => false,
  isMaximized: () => false,
  isFocused: () => true,
  webContents: { getZoomFactor: () => 1 }
};

electron.remote = Object.assign({}, electron.remote, {
  app: electron.app,
  clipboard: electron.clipboard,
  dialog: electron.dialog,
  shell: electron.shell,
  nativeTheme: electron.nativeTheme,
  webFrame: electron.webFrame,
  screen: electron.screen,
  Menu: electron.Menu,
  safeStorage: electron.safeStorage,
  systemPreferences: electron.systemPreferences,
  getCurrentWindow: () => fenetreCourante,
  getCurrentWebContents: () => ({ session: { fromPartition: () => ({}) } })
});

journaliser('preload :', 'electron ' + process.versions.electron + ', remote minimal installé');
