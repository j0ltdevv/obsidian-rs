/*
 * Preload du harnais (code écrit pour ce diagnostic) : Electron 45 a retiré le
 * module `remote`, or le renderer d'Obsidian 1.14.4 l'utilise (il tente
 * `require('@electron/remote')`). Ce preload installe un `remote` minimal
 * avant le chargement de la page.
 */
const electron = require('electron');

/* Méthodes d'événement : stubs sans effet. */
const emitter = () => ({
  on: function () { return this; },
  once: function () { return this; },
  addListener: function () { return this; },
  removeListener: function () { return this; },
  removeAllListeners: function () { return this; },
  emit: function () { return false; },
  setMaxListeners: function () { return this; },
  listeners: function () { return []; },
});

const webContents = Object.assign(emitter(), {
  getZoomFactor: () => 1,
  setZoomFactor: () => {},
  getVisualZoomLevel: () => 0,
  setVisualZoomLevel: () => {},
  getZoomLevel: () => 0,
  setZoomLevel: () => {},
  isDestroyed: () => false,
  getURL: () => 'app://obsidian.md/index.html',
  send: () => {},
  session: { fromPartition: () => ({}) },
});

const currentWindow = Object.assign(emitter(), {
  id: 1,
  minimizable: () => true,
  maximizable: () => true,
  closable: () => true,
  fullScreenable: () => true,
  resizable: () => true,
  isMaximized: () => false,
  isFullScreen: () => false,
  isFocused: () => true,
  isMinimized: () => false,
  isVisible: () => true,
  isDestroyed: () => false,
  getBounds: () => ({ x: 0, y: 0, width: 1400, height: 900 }),
  getSize: () => [1400, 900],
  getPosition: () => [0, 0],
  setTitle: () => {},
  show: () => {},
  focus: () => {},
  restore: () => {},
  close: () => {},
  setMenuBarVisibility: () => {},
  webContents,
});

const app = Object.assign(emitter(), {
  getVersion: () => '1.14.4',
  getName: () => 'Obsidian',
  getAppPath: () => '/',
  getPath: () => '/tmp',
  getPreferredSystemLanguages: () => ['fr-FR'],
  getLocale: () => 'fr-FR',
  getSystemLocale: () => 'fr-FR',
  getLoginItemSettings: () => ({ openAtLogin: false }),
  isPackaged: false,
});

const nativeTheme = Object.assign(emitter(), {
  shouldUseDarkColors: true,
  shouldUseHighContrastColors: false,
  themeSource: 'system',
});

const clipboard = Object.assign(emitter(), {
  readText: () => '',
  writeText: () => {},
});

const shell = Object.assign(emitter(), {
  openExternal: () => {},
  showItemInFolder: () => {},
  openPath: () => {},
});

const dialog = Object.assign(emitter(), {
  showMessageBox: () => Promise.resolve({ response: 0 }),
  showOpenDialog: () => Promise.resolve({ canceled: true, filePaths: [] }),
  showSaveDialog: () => Promise.resolve({ canceled: true, filePath: '' }),
  showErrorBox: () => {},
});

const webFrame = Object.assign(emitter(), {
  setZoomFactor: () => {},
  getZoomFactor: () => 1,
  setZoomLevel: () => {},
  getZoomLevel: () => 0,
  setVisualZoomLevel: () => {},
  getVisualZoomLevel: () => 0,
});

const screen = Object.assign(emitter(), {
  getAllDisplays: () => [{ workArea: { x: 0, y: 0, width: 1400, height: 900 }, size: { width: 1400, height: 900 } }],
  getPrimaryDisplay: () => ({ workArea: { x: 0, y: 0, width: 1400, height: 900 }, size: { width: 1400, height: 900 } }),
});

const Menu = Object.assign(emitter(), {
  buildFromTemplate: () => ({ popup: () => {} }),
  setApplicationMenu: () => {},
});

const safeStorage = Object.assign(emitter(), {
  isEncryptionAvailable: () => false,
  encryptString: () => Buffer.from(''),
  decryptString: () => '',
});

const systemPreferences = Object.assign(emitter(), {
  isDarkMode: () => true,
  getMediaColorScheme: () => 'dark',
  getUserDefault: () => undefined,
});

electron.remote = {
  app,
  getCurrentWindow: () => currentWindow,
  getCurrentWebContents: () => webContents,
  clipboard,
  shell,
  dialog,
  nativeTheme,
  webFrame,
  screen,
  Menu,
  safeStorage,
  systemPreferences,
};

console.log('[preload] remote minimal installé');
