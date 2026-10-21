/*
 * Harnais de démarrage Electron (code propre, écrit pour ce diagnostic) :
 * il remplace le processus principal d'Obsidian par un jeu de handlers IPC
 * minimal, et charge le renderer réel (`asar-x1/index.html`) dans une
 * BrowserWindow avec `nodeIntegration` actif.
 *
 * Aucun code d'Obsidian n'est recopié : la chaîne d'acceptation est lue
 * dans leur `main.js` au moment de l'exécution.
 */
const { app, BrowserWindow, ipcMain, protocol } = require('electron');
const fs = require('fs');
const path = require('path');

const ROOT = '/data/home/obsidian-rea';
const ASAR_X = path.join(ROOT, 'asar-x1');
const VAULT = path.join(ROOT, 'vault');
const SCHEME = process.env.HARNESS_SCHEME || 'file';

let terms = '';
try {
  const src = fs.readFileSync(path.join(ASAR_X, 'main.js'), 'utf8');
  const m = /"I understand and agree that I am not allowed to distribute[^"]*"/.exec(src);
  if (m) { terms = m[0].slice(1, -1); }
} catch (err) {
  console.log('[harnais] chaîne d\'acceptation introuvable :', err.message);
}
console.log('[harnais] chaîne d\'acceptation :', terms.length, 'caractères');

const REPLIES = {
  vault: { id: 'harnais', path: VAULT },
  'vault-list': { vaults: [{ id: 'harnais', path: VAULT, name: 'vault', open: true, ts: 0 }] },
  'vault-open': { id: 'harnais', path: VAULT },
  'get-sandbox-vault-path': VAULT,
  'file-url': 'file://' + VAULT,
  'documents-dir': VAULT,
  'desktop-dir': VAULT,
  version: '1.14.4',
  terms: terms,
  policy: {},
  starter: null,
  frame: { width: 1400, height: 900, x: 0, y: 0, maximized: false },
  'is-quitting': false,
  'is-closing': false,
  sandbox: false,
  'disable-gpu': true,
  'disable-update': true,
  update: false,
  'insider-build': false,
  'check-update': false,
  'latest-public-version': '1.14.4',
  relaunch: '',
  help: '',
  trash: false,
  cli: '',
  'set-language': '',
  'set-icon': '',
  'get-icon': '',
  'copy-asar': '',
  'adblock-lists': [],
  'adblock-frequency': 0,
};

if (SCHEME === 'app') {
  protocol.registerSchemesAsPrivileged([
    { scheme: 'app', privileges: { standard: true, secure: true, supportFetchAPI: true } },
  ]);
}

let remoteModule = null;
try {
  remoteModule = require('@electron/remote/main');
} catch (err) {
  console.log('[harnais] @electron/remote/main absent :', err.message);
}

app.whenReady().then(() => {
  if (remoteModule && remoteModule.enable) {
    try {
      remoteModule.enable();
      console.log('[harnais] @electron/remote activé');
    } catch (err) {
      console.log('[harnais] @electron/remote non activable :', err.message);
    }
  }
  Object.keys(REPLIES).forEach((channel) => {
    ipcMain.on(channel, (event) => { event.returnValue = REPLIES[channel]; });
  });
  ipcMain.on('context-menu', () => {});
  ipcMain.on('request-url', (event) => { event.returnValue = ''; });

  if (SCHEME === 'app') {
    protocol.handle('app', async (request) => {
      const url = new URL(request.url);
      const file = path.join(ASAR_X, decodeURIComponent(url.pathname));
      const data = fs.readFileSync(file);
      return new Response(data, { headers: { 'Content-Type': 'text/html' } });
    });
  }

  const win = new BrowserWindow({
    width: 1400,
    height: 900,
    show: false,
    webPreferences: {
      contextIsolation: false,
      nodeIntegration: true,
      nodeIntegrationInWorker: true,
      spellcheck: true,
      webviewTag: true,
      preload: path.join(__dirname, 'preload-harness.js'),
    },
  });

  win.webContents.on('console-message', (...args) => {
    console.log('[renderer]', JSON.stringify(args));
  });
  win.webContents.on('render-process-gone', (...args) => {
    console.log('[renderer] processus disparu', JSON.stringify(args));
  });
  win.webContents.on('did-fail-load', (...args) => {
    console.log('[renderer] chargement impossible', JSON.stringify(args));
  });

  const url = SCHEME === 'app' ? 'app://obsidian.md/index.html' : 'file://' + path.join(ASAR_X, 'index.html');
  console.log('[harnais] chargement de', url);
  win.loadURL(url);
  win.once('ready-to-show', () => { win.show(); console.log('[harnais] fenêtre affichée'); });
  setTimeout(() => {
    console.log('[harnais] URL finale :', win.webContents.getURL());
    console.log('[harnais] titre :', win.getTitle());
  }, 12000);
});
