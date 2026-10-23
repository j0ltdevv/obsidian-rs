/*
 * Lanceur du mode « compat » (Electron) : démarre le moteur Rust obsy,
 * ouvre une BrowserWindow avec nodeIntegration, sert le renderer réel
 * d'Obsidian (index.html extrait de l'ASAR) en HTTP local en injectant
 * `shim/browser-shim.js` avant `app.js`, et répond aux canaux IPC du boot.
 *
 * Aucun code d'Obsidian n'est recopié : la chaîne d'acceptation et le reste
 * sont lus dans l'application au moment de l'exécution. `shim/compat-electron.js`
 * est réutilisé pour la surface `electron.remote` et la lecture de la chaîne.
 *
 * Ce fichier est un module ES (le `shim/package.json` porte `"type":"module"`).
 *
 * Arguments (après le chemin du script) :
 *   --vault <chemin>      racine du vault (obligatoire)
 *   --asar <chemin>       dossier d'ASAR extrait / fichier .asar (sinon détecté
 *                         via ~/.config/obsidian/obsidian.asar)
 *   --engine-bin <fichier> binaire du moteur Rust (sinon OBSIDIAN_ENGINE_BIN,
 *                         puis `obsy-engine` dans PATH)
 *   --engine-port <N>     port du moteur (défaut 27123)
 *   --static-port <N>     port du serveur statique local (défaut 8823)
 */
import { app, BrowserWindow, ipcMain } from 'electron';
import { spawn } from 'child_process';
import http from 'http';
import path from 'path';
import fs from 'fs';
import os from 'os';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

/* -------------------------------------------------------------------- */
/* Arguments                                                             */
/* -------------------------------------------------------------------- */

function parseArgs(argv) {
  const opts = {};
  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === '--vault' || arg === '--asar' || arg === '--engine-bin' ||
        arg === '--engine-port' || arg === '--static-port') {
      opts[arg.slice(2)] = argv[i + 1];
      i += 1;
    }
  }
  return opts;
}

const opt = parseArgs(process.argv.slice(2));

const VAULT = path.resolve(opt.vault || '');
if (!VAULT || !fs.existsSync(VAULT)) {
  console.error('[compat] vault introuvable (--vault) :', VAULT);
  process.exit(1);
}

const ENGINE_PORT = parseInt(opt['engine-port'], 10) || 27123;
const STATIC_PORT = parseInt(opt['static-port'], 10) || 8823;
const ENGINE_ORIGIN = 'http://127.0.0.1:' + ENGINE_PORT;
let ASAR_DIR = null; // dossier d'ASAR extrait (servi en HTTP)
let ASAR_FILE = null; // chemin du .asar (pour lire la chaîne d'acceptation)

/* -------------------------------------------------------------------- */
/* ASAR : résolution + extraction                                        */
/* -------------------------------------------------------------------- */

/**
 * Parse l'en-tête ASAR (format Electron). Identique à `parseAsar` de
 * `shim/compat-electron.js` : renvoie `{ header, dataStart }` ou `null`.
 */
function parseAsarHeader(octets) {
  if (octets.length < 8 + 8 + 8) return null;
  const vue = new DataView(octets.buffer, octets.byteOffset, octets.byteLength);
  const taillePickleTaille = vue.getUint32(0, true);
  const tailleHeaderBuf = vue.getUint32(4, true);
  if (taillePickleTaille !== 4) return null;
  const debutHeader = 8;
  if (debutHeader + tailleHeaderBuf > octets.length) return null;
  const headerVue = new DataView(octets.buffer, octets.byteOffset + debutHeader, tailleHeaderBuf);
  const tailleChaine = headerVue.getUint32(4, true);
  if (tailleChaine <= 0 || debutHeader + 8 + tailleChaine > octets.length) return null;
  const jsonBrut = new Uint8Array(octets.buffer, octets.byteOffset + debutHeader + 8, tailleChaine);
  let header;
  try {
    header = JSON.parse(Buffer.from(jsonBrut).toString('utf8'));
  } catch (e) {
    return null;
  }
  if (!header || typeof header !== 'object' || !header.files) return null;
  return { header, dataStart: debutHeader + tailleHeaderBuf };
}

function extraireFichier(octets, dataStart, entree) {
  const offset = parseInt(String(entree.offset), 10);
  const taille = entree.size;
  if (!Number.isFinite(offset) || offset < 0) return null;
  const debut = dataStart + offset;
  if (debut < dataStart || debut + taille > octets.length) return null;
  return octets.slice(debut, debut + taille);
}

function ecrireArborescence(entrees, prefixe, octets, dataStart, sortie) {
  for (const nom of Object.keys(entrees)) {
    const entree = entrees[nom];
    const relatif = prefixe ? prefixe + '/' + nom : nom;
    if (entree.files) {
      const dossier = path.join(sortie, relatif);
      fs.mkdirSync(dossier, { recursive: true });
      ecrireArborescence(entree.files, relatif, octets, dataStart, sortie);
    } else {
      const bytes = extraireFichier(octets, dataStart, entree);
      if (bytes !== null) {
        fs.writeFileSync(path.join(sortie, relatif), Buffer.from(bytes));
      }
    }
  }
}

function extraireAsar(asarPath) {
  const octets = fs.readFileSync(asarPath);
  const parse = parseAsarHeader(octets);
  if (!parse) {
    throw new Error('en-tête ASAR invalide : ' + asarPath);
  }
  const sortie = fs.mkdtempSync(path.join(os.tmpdir(), 'obsidian-compat-'));
  ecrireArborescence(parse.header.files, '', octets, parse.dataStart, sortie);
  return sortie;
}

const CHEMINS_ASAR_SYSTEME = [
  '/usr/lib/obsidian/resources/app.asar',
  '/opt/Obsidian/resources/app.asar',
  '/usr/share/obsidian/resources/app.asar',
  '/snap/obsidian/current/resources/app.asar',
];

function resoudreAsar() {
  if (opt.asar) {
    const p = path.resolve(opt.asar);
    if (!fs.existsSync(p)) return;
    if (fs.statSync(p).isDirectory()) {
      ASAR_DIR = p;
      return;
    }
    ASAR_FILE = p;
    ASAR_DIR = extraireAsar(p);
    return;
  }
  for (const chemin of CHEMINS_ASAR_SYSTEME) {
    if (fs.existsSync(chemin)) {
      ASAR_FILE = chemin;
      ASAR_DIR = extraireAsar(chemin);
      return;
    }
  }
  const candidat = path.join(os.homedir(), '.config', 'obsidian', 'obsidian.asar');
  if (fs.existsSync(candidat)) {
    ASAR_FILE = candidat;
    ASAR_DIR = extraireAsar(candidat);
  }
}

/**
 * Lit la chaîne d'acceptation (284 caractères) depuis `app.js`, soit dans
 * l'ASAR extrait, soit dans le fichier `.asar`. Aucun texte n'est stocké.
 */
function lireChaineAcceptation() {
  let texte = null;
  if (ASAR_DIR) {
    const appJs = path.join(ASAR_DIR, 'app.js');
    if (fs.existsSync(appJs)) texte = fs.readFileSync(appJs, 'utf8');
  }
  if (texte === null && ASAR_FILE) {
    try {
      const octets = fs.readFileSync(ASAR_FILE);
      const parse = parseAsarHeader(octets);
      if (parse) {
        const recherche = (entrees) => {
          for (const nom of Object.keys(entrees)) {
            const e = entrees[nom];
            if (e.files) { const t = recherche(e.files); if (t) return t; }
            else if (nom === 'app.js') return e;
          }
          return null;
        };
        const entree = recherche(parse.header.files);
        if (entree) {
          const bytes = extraireFichier(octets, parse.dataStart, entree);
          if (bytes !== null) texte = Buffer.from(bytes).toString('utf8');
        }
      }
    } catch (e) { /* silencieux */ }
  }
  if (texte === null) return null;
  // La constante est localisée **par la comparaison qu'en fait le renderer**
  // (`"…" !== ipcRenderer.sendSync("terms")`) : on récupère exactement le
  // littéral comparé, quelle que soit sa longueur, sans recopier le texte
  // d'Obsidian dans ce dépôt.
  const match = /"((?:[^"\\]|\\.){40,})"!==[^;]{0,160}?sendSync\(\s*["']terms["']\s*\)/.exec(texte);
  if (!match) return null;
  try {
    return JSON.parse('"' + match[1] + '"');
  } catch (e) {
    return match[1];
  }
}

resoudreAsar();
if (!ASAR_DIR) {
  console.error('[compat] ASAR d\'Obsidian introuvable (--asar ou ~/.config/obsidian/obsidian.asar)');
  process.exit(1);
}
const TERMS = lireChaineAcceptation();
if (TERMS === null) {
  console.error('[compat] chaîne d\'acceptation introuvable dans l\'ASAR : ' +
    'application Obsidian non installée ou ASAR invalide');
  process.exit(1);
}

/* -------------------------------------------------------------------- */
/* Moteur Rust                                                            */
/* -------------------------------------------------------------------- */

let engineChild = null;

function resoudreEngineBin() {
  if (opt['engine-bin']) return path.resolve(opt['engine-bin']);
  if (process.env.OBSIDIAN_ENGINE_BIN) return process.env.OBSIDIAN_ENGINE_BIN;
  const cherche = ['obsy-engine'];
  if (process.env.PATH) {
    for (const dir of process.env.PATH.split(path.delimiter)) {
      if (!dir) continue;
      const p = path.join(dir, 'obsy-engine');
      if (fs.existsSync(p)) return p;
    }
  }
  return cherche[0];
}

function demarrerMoteur() {
  const binaire = resoudreEngineBin();
  engineChild = spawn(binaire, ['--root', VAULT, '--port', String(ENGINE_PORT)], {
    stdio: ['ignore', 'pipe', 'pipe'],
  });
  engineChild.stdout.on('data', (d) => process.stderr.write('[engine] ' + d));
  engineChild.stderr.on('data', (d) => process.stderr.write('[engine] ' + d));
  engineChild.on('exit', (code, signal) => {
    process.stderr.write('[compat] moteur arrêté (code=' + code + ', signal=' + signal + ')\n');
  });
  return engineChild;
}

function attendreSante(port, timeoutMs) {
  const fin = Date.now() + timeoutMs;
  return new Promise((resolve) => {
    const tenter = () => {
      if (Date.now() > fin) return resolve(false);
      const req = http.get({ host: '127.0.0.1', port, path: '/health', timeout: 1000 }, (res) => {
        let corps = '';
        res.on('data', (d) => { corps += d; });
        res.on('end', () => {
          try {
            if (res.statusCode === 200 && JSON.parse(corps).ok) return resolve(true);
          } catch (e) { /* pas encore prêt */ }
          setTimeout(tenter, 250);
        });
      });
      req.on('error', () => setTimeout(tenter, 250));
      req.on('timeout', () => { req.destroy(); setTimeout(tenter, 250); });
    };
    tenter();
  });
}

function arreterMoteur() {
  if (!engineChild) return;
  try { engineChild.kill('SIGTERM'); } catch (e) { /* ignore */ }
  engineChild = null;
}

/* -------------------------------------------------------------------- */
/* Serveur statique + injection du shim                                  */
/* -------------------------------------------------------------------- */

const MIME = {
  '.html': 'text/html; charset=utf-8',
  '.js': 'application/javascript',
  '.mjs': 'application/javascript',
  '.css': 'text/css',
  '.json': 'application/json',
  '.png': 'image/png',
  '.jpg': 'image/jpeg',
  '.gif': 'image/gif',
  '.svg': 'image/svg+xml',
  '.ico': 'image/x-icon',
  '.woff': 'font/woff',
  '.woff2': 'font/woff2',
  '.ttf': 'font/ttf',
  '.map': 'application/json',
};

function fabriquerInjection() {
  // Surface `remote` + lecture ASAR (pur JS) : injectée en script classique,
  // avant `app.js` (qui est `defer`). La chaîne d'acceptation, elle, est
  // calculée par le processus principal (`TERMS` ci-dessus) et injectée
  // comme valeur déjà calculée — jamais recopiée ni relue dans la page.
  const compatSource = fs
    .readFileSync(path.join(__dirname, 'compat-electron.js'), 'utf8')
    .replace(/^export\s+/gm, '');
  const termsJson = JSON.stringify(TERMS);
  return (
    '<script>\n(function () {\n' +
    '  var host = window;\n' +
    // La fermeture de fenêtre est le symptôme du boot qui échoue : on
    // intercepte pour connaître la pile d'appels au lieu de perdre la page.
    '  var closeReel = host.close;\n' +
    '  host.close = function () {\n' +
    '    var pile = "";\n' +
    '    try { pile = (new Error("window.close()")).stack || ""; } catch (e) { pile = String(e); }\n' +
    '    host.__OBSIDIAN_CLOSE_STACK__ = pile;\n' +
    '    if (host.console) { host.console.log("[compat] window.close() intercepté :\\n" + pile); }\n' +
    '  };\n' +
    '  host.__OBSIDIAN_APP_ASAR__ = ' + (ASAR_FILE ? JSON.stringify(ASAR_FILE) : 'undefined') + ';\n' +
    '  host.__OBSIDIAN_ENGINE_PORT = ' + ENGINE_PORT + ';\n' +
    '  host.__OBSIDIAN_ENGINE_ORIGIN = ' + JSON.stringify(ENGINE_ORIGIN) + ';\n' +
    '  host.__OBSIDIAN_TERMS__ = ' + termsJson + ';\n' +
    compatSource + '\n' +
    '  host.__OBSIDIAN_COMPAT__ = {\n' +
    '    creerSurfaceRemote: creerSurfaceRemote,\n' +
    '    lireChaineAcceptation: function () {\n' +
    '      return host.__OBSIDIAN_TERMS__;\n' +
    '    }\n' +
    '  };\n' +
    '})();\n</script>\n' +
    '<script src="/__shim/browser-shim.js"></script>\n'
  );
}

const INJECTION = fabriquerInjection();

function reecrireIndex(html) {
  const entete = html.match(/<head[^>]*>/i);
  if (entete && entete.index !== undefined) {
    return html.slice(0, entete.index) + entete[0] + INJECTION + html.slice(entete.index + entete[0].length);
  }
  const corps = html.match(/<body[^>]*>/i);
  if (corps && corps.index !== undefined) {
    return html.slice(0, corps.index) + corps[0] + INJECTION + html.slice(corps.index + corps[0].length);
  }
  return INJECTION + html;
}

/** Répond à une requête du serveur statique (page de leur renderer + shim). */
function repondre(req, res) {
  let url = req.url.split('?')[0];
  if (url === '/') url = '/index.html';
  if (url === '/__shim/browser-shim.js' || url === '/browser-shim.js') {
    const fichier = path.join(__dirname, 'browser-shim.js');
    const data = fs.readFileSync(fichier);
    res.writeHead(200, { 'Content-Type': 'application/javascript' });
    return res.end(data);
  }
  if (!ASAR_DIR) {
    res.writeHead(500, { 'Content-Type': 'text/plain; charset=utf-8' });
    return res.end('application Obsidian introuvable (--asar)');
  }
  const cible = path.join(ASAR_DIR, decodeURIComponent(url).replace(/^\/+/, ''));
  if (ASAR_DIR !== cible && !cible.startsWith(ASAR_DIR + path.sep)) {
    res.writeHead(403, { 'Content-Type': 'text/plain; charset=utf-8' });
    return res.end('accès refusé');
  }
  let stat;
  try { stat = fs.statSync(cible); } catch (e) {
    res.writeHead(404, { 'Content-Type': 'text/plain; charset=utf-8' });
    return res.end('introuvable');
  }
  if (stat.isDirectory()) {
    res.writeHead(301, { Location: url + '/' });
    return res.end();
  }
  const bas = url.toLowerCase();
  let data;
  if (bas === '/index.html') {
    data = Buffer.from(reecrireIndex(fs.readFileSync(cible, 'utf8')), 'utf8');
  } else {
    data = fs.readFileSync(cible);
  }
  res.writeHead(200, { 'Content-Type': MIME[path.extname(cible)] || 'application/octet-stream' });
  res.end(data);
}

function demarrerServeur() {
  let port = STATIC_PORT;
  return new Promise((resolve, reject) => {
    const tenter = () => {
      const serveur = http.createServer((req, res) => {
        process.stderr.write('[compat] requete ' + req.url + '\n');
        try {
          repondre(req, res);
        } catch (err) {
          // Une exception ici laisserait la requête pendante (aucune
          // réponse) : on répond toujours, avec le message réel.
          process.stderr.write('[compat] erreur serveur : ' + (err && err.stack ? err.stack : err) + '\n');
          res.writeHead(500, { 'Content-Type': 'text/plain; charset=utf-8' });
          res.end('erreur interne');
        }
      });
      serveur.on('error', (err) => {
        if (err.code === 'EADDRINUSE' && port < STATIC_PORT + 20) {
          port += 1;
          return tenter();
        }
        reject(err);
      });
      serveur.listen(port, '127.0.0.1', () => resolve({ serveur, port }));
    };
    tenter();
  });
}

/* -------------------------------------------------------------------- */
/* IPC (canaux du boot)                                                  */
/* -------------------------------------------------------------------- */

function idStable(racine) {
  let h = 5381;
  for (const c of racine) h = ((h << 5) + h + c.charCodeAt(0)) | 0;
  return 'vault-' + (h >>> 0).toString(16);
}

const ID_VAULT = idStable(VAULT);

const REPLIES = {
  vault: { id: ID_VAULT, path: VAULT },
  'vault-list': { vaults: [{ id: ID_VAULT, path: VAULT, name: 'vault', open: true, ts: 0 }] },
  'vault-open': { id: ID_VAULT, path: VAULT },
  'get-sandbox-vault-path': VAULT,
  'file-url': 'file://' + VAULT,
  'documents-dir': VAULT,
  'desktop-dir': VAULT,
  version: '1.14.4',
  terms: TERMS,
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

function enregistrerIpc() {
  Object.keys(REPLIES).forEach((canal) => {
    ipcMain.on(canal, (event) => { event.returnValue = REPLIES[canal]; });
  });
  ipcMain.on('context-menu', () => {});
  ipcMain.on('request-url', (event) => { event.returnValue = ''; });
}

/* -------------------------------------------------------------------- */
/* Démarrage                                                             */
/* -------------------------------------------------------------------- */

async function main() {
  process.stderr.write('[compat] vault=' + VAULT + ' asar=' + ASAR_DIR + '\n');
  process.stderr.write('[compat] moteur=' + resoudreEngineBin() + ' port=' + ENGINE_PORT + '\n');

  demarrerMoteur();
  const sain = await attendreSante(ENGINE_PORT, 15000);
  if (!sain) {
    console.error('[compat] moteur injoignable sur /health:' + ENGINE_PORT);
    process.exit(1);
  }
  process.stderr.write('[compat] moteur prêt\n');

  const { serveur, port } = await demarrerServeur();
  process.stderr.write('[compat] serveur statique sur http://127.0.0.1:' + port + '\n');

  enregistrerIpc();

  const win = new BrowserWindow({
    width: 1400,
    height: 900,
    show: false,
    webPreferences: {
      contextIsolation: false,
      nodeIntegration: true,
      nodeIntegrationInWorker: true,
      // Preload de diagnostic : journalise les erreurs du renderer (une
      // fenêtre qui se ferme au boot laisse sinon un journal muet) et installe
      // la surface `remote` minimale qu'Electron 45 ne fournit plus.
      preload: path.join(__dirname, 'compat-preload.js'),
      spellcheck: true,
      webviewTag: true,
    },
  });

  win.webContents.on('console-message', (e, niveau, message, ligne, source) => {
    if (e && typeof e === 'object' && 'level' in e) {
      niveau = e.level; message = e.message; ligne = e.lineNumber; source = e.sourceId;
    }
    const libelle = { 0: 'verbose', 1: 'info', 2: 'warning', 3: 'error' }[niveau] || niveau;
    process.stderr.write('[renderer] ' + libelle + ': ' + message + ' (' + source + ':' + ligne + ')\n');
  });
  win.webContents.on('render-process-gone', (_e, details) => {
    process.stderr.write('[compat] processus renderer disparu: ' + JSON.stringify(details) + '\n');
  });
  win.webContents.on('did-fail-load', (_e, code, desc) => {
    process.stderr.write('[compat] chargement impossible: ' + code + ' ' + desc + '\n');
  });
  // Sonde : que voit réellement la page (shim installé ? canal « vault » ?)
  win.webContents.on('dom-ready', () => {
    const sonde = [
      'JSON.stringify({',
      'shim: typeof window.require,',
      'moteur: !!(window.__OBSIDIAN_ENGINE__),',
      'compat: !!(window.__OBSIDIAN_COMPAT__),',
      'vault: (function(){try{return window.require("electron").ipcRenderer.sendSync("vault")}catch(e){return "ERREUR: "+e.message}})(),',
      'racine: (function(){try{return window.__OBSIDIAN_ENGINE__.call("ipc.vault.root", [])}catch(e){return "ERREUR: "+e.message}})(),',
      'existe: (function(){try{return window.require("fs").existsSync("/tmp/vault-compat")}catch(e){return "ERREUR: "+e.message}})()',
      '})'
    ].join('\n');
    win.webContents
      .executeJavaScript(sonde, true)
      .then((r) => process.stderr.write('[compat] sonde : ' + r + '\n'))
      .catch((e) => process.stderr.write('[compat] sonde impossible : ' + e + '\n'));
  });

  const url = 'http://127.0.0.1:' + port + '/index.html';
  process.stderr.write('[compat] chargement de ' + url + '\n');
  await win.loadURL(url);
  win.once('ready-to-show', () => win.show());

  process.stderr.write('[compat] fenêtre affichée\n');
  setTimeout(() => {
    process.stderr.write('[compat] titre: ' + win.getTitle() + '\n');
  }, 12000);

  serveur.unref();
  // L'enfant moteur reste référencé ; `will-quit` l'arrête proprement.
}

app.on('window-all-closed', () => {
  app.quit();
});

app.on('will-quit', () => {
  arreterMoteur();
});

process.on('exit', () => {
  arreterMoteur();
});

app.whenReady().then(main).catch((err) => {
  console.error('[compat] erreur fatale:', err);
  arreterMoteur();
  process.exit(1);
});
