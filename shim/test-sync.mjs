#!/usr/bin/env node
/**
 * Test du shim navigateur (`shim/browser-shim.js`) contre le moteur Rust.
 *
 * Le test est « jsdom-free » : le shim est exécuté dans un contexte `vm`
 * node, avec un `XMLHttpRequest` factice. Ce factice n'est pas une simulation :
 * la requête part réellement en HTTP vers le moteur Rust démarré par le
 * test. Le transport est synchrone côté appelant ; côté node, la requête est
 * exécutée dans un *worker* (le fil principal est bloqué par
 * `Atomics.wait`, la boucle d'événements ne peut donc pas traiter la
 * réponse).
 *
 * Usage :
 *   node shim/test-sync.mjs [--engine-bin <chemin>] [--port <N>]
 */

import { spawn } from 'node:child_process';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createContext, runInContext } from 'node:vm';
import { Worker } from 'node:worker_threads';
import { creerSurfaceRemote, lireChaineAcceptation } from './compat-electron.js';

const HERE = dirname(fileURLToPath(import.meta.url));
const REPO = resolve(HERE, '..');

/* ------------------------------------------------------------------ */
/* Utilitaires de test                                                 */
/* ------------------------------------------------------------------ */

let passed = 0;
let failed = 0;

function check(label, fn) {
  try {
    fn();
    passed += 1;
    console.log('  OK    ' + label);
  } catch (err) {
    failed += 1;
    console.log('  ÉCHEC ' + label + ' : ' + (err && err.message ? err.message : err));
  }
}

function assert(condition, message) {
  if (!condition) {
    throw new Error(message || 'condition fausse');
  }
}

function assertEqual(actual, expected, message) {
  const left = JSON.stringify(actual);
  const right = JSON.stringify(expected);
  if (left !== right) {
    throw new Error((message || 'valeur inattendue') + ' : ' + left + ' != ' + right);
  }
}

/**
 * Construit un faux ASAR (format Electron identique à celui de
 * `@electron/asar`) contenant un seul fichier `app.js` de contenu donné, et
 * renvoie ses octets.
 */
function construireFauxAsar(contenuAppJs) {
  const corps = Buffer.from(contenuAppJs, 'utf8');
  const json = JSON.stringify({
    files: {
      'app.js': { size: corps.length, offset: '0' }
    }
  });
  const tailleChaine = Buffer.byteLength(json, 'utf8');

  // Pickle d'en-tête : [payloadSize][tailleChaine][JSON].
  const headerBuf = Buffer.alloc(8 + tailleChaine);
  headerBuf.writeUInt32LE(4 + tailleChaine, 0);
  headerBuf.writeUInt32LE(tailleChaine, 4);
  headerBuf.write(json, 8, 'utf8');

  // Size pickle : [payloadSize=4][tailleHeaderBuf].
  const sizeBuf = Buffer.alloc(8);
  sizeBuf.writeUInt32LE(4, 0);
  sizeBuf.writeUInt32LE(headerBuf.length, 4);

  return Buffer.concat([sizeBuf, headerBuf, corps]);
}

/* ------------------------------------------------------------------ */
/* Transport synchrone (XHR factice sur un worker)                    */
/* ------------------------------------------------------------------ */

const WORKER_SOURCE = `
const { parentPort, workerData } = require('node:worker_threads');
const http = require('node:http');

const header = new Int32Array(workerData.headerSab);
const body = new Uint8Array(workerData.bodySab);

parentPort.on('message', (message) => {
  if (message.type === 'ping') {
    parentPort.postMessage({ type: 'pong' });
    return;
  }
  const request = http.request(
    message.url,
    { method: message.method, headers: message.headers },
    (response) => {
      const chunks = [];
      response.on('data', (chunk) => chunks.push(chunk));
      response.on('end', () => {
        const payload = Buffer.concat(chunks);
        const length = Math.min(payload.length, body.length);
        body.set(payload.subarray(0, length));
        header[1] = response.statusCode;
        header[2] = length;
        Atomics.store(header, 0, 1);
        Atomics.notify(header, 0);
      });
    }
  );
  request.on('error', () => {
    header[1] = 0;
    header[2] = 0;
    Atomics.store(header, 0, 2);
    Atomics.notify(header, 0);
  });
  request.end(message.payload);
});
`;

const headerSab = new SharedArrayBuffer(16);
const header = new Int32Array(headerSab);
const bodySab = new SharedArrayBuffer(4 * 1024 * 1024);
const body = new Uint8Array(bodySab);

const worker = new Worker(WORKER_SOURCE, {
  eval: true,
  workerData: { headerSab, bodySab }
});

await new Promise((resolvePromise) => {
  worker.once('message', (message) => {
    if (message.type === 'pong') {
      resolvePromise();
    }
  });
  worker.postMessage({ type: 'ping' });
});

/** Requête HTTP réellement exécutée, mais attendue de façon synchrone. */
function syncRequest(url, method, headers, payload) {
  Atomics.store(header, 0, 0);
  worker.postMessage({ type: 'request', url, method, headers, payload });
  const outcome = Atomics.wait(header, 0, 0, 15000);
  if (outcome === 'timed-out') {
    throw new Error('délai dépassé pour ' + url);
  }
  const state = Atomics.load(header, 0);
  const status = Atomics.load(header, 1);
  const length = Atomics.load(header, 2);
  if (state === 2) {
    throw new Error('échec réseau pour ' + url);
  }
  return { status, text: new TextDecoder().decode(body.subarray(0, length)) };
}

/** Journal des requêtes émises par le shim (opérations réellement envoyées). */
const sentRequests = [];

/** `XMLHttpRequest` factice : interroge le vrai moteur, en mode synchrone. */
class FakeXMLHttpRequest {
  constructor() {
    this.status = 0;
    this.responseText = '';
    this.readyState = 0;
    this.async = true;
    this.method = '';
    this.url = '';
    this.headers = {};
  }

  open(method, url, async) {
    this.method = method;
    this.url = url;
    this.async = async !== false;
    this.readyState = 1;
  }

  setRequestHeader(name, value) {
    this.headers[name] = value;
  }

  send(payload) {
    const body = payload === undefined || payload === null ? '' : String(payload);
    sentRequests.push({
      method: this.method,
      url: this.url,
      async: this.async,
      contentType: this.headers['Content-Type'] || '',
      op: (() => {
        try {
          return JSON.parse(body).op;
        } catch (err) {
          return null;
        }
      })()
    });
    const result = syncRequest(this.url, this.method, this.headers, body);
    this.status = result.status;
    this.responseText = result.text;
    this.readyState = 4;
  }
}

/* ------------------------------------------------------------------ */
/* Moteur Rust                                                         */
/* ------------------------------------------------------------------ */

function findEngineBinary() {
  if (process.env.OBSIDIAN_ENGINE_BIN) {
    return process.env.OBSIDIAN_ENGINE_BIN;
  }
  const candidates = [
    join(REPO, 'target', 'release', 'obsidian-engine'),
    join(REPO, 'target', 'debug', 'obsidian-engine')
  ];
  for (const candidate of candidates) {
    if (existsSync(candidate)) {
      return candidate;
    }
  }
  throw new Error(
    'binaire obsidian-engine introuvable (construisez-le avec `cargo build --bin obsidian-engine`, ou settez OBSIDIAN_ENGINE_BIN)'
  );
}

/** Démarre le moteur sur un vault temporaire et retourne son port. */
async function startEngine(binary, vault) {
  const child = spawn(binary, ['--root', vault, '--port', '0'], {
    stdio: ['ignore', 'pipe', 'pipe']
  });
  child.stdout.setEncoding('utf8');
  child.stderr.setEncoding('utf8');
  let stderr = '';
  child.stderr.on('data', (chunk) => {
    stderr += chunk;
  });
  const port = await new Promise((resolvePromise, rejectPromise) => {
    let buffer = '';
    const timer = setTimeout(() => {
      rejectPromise(new Error('le moteur n’a pas démarré : ' + stderr));
    }, 15000);
    child.stdout.on('data', (chunk) => {
      buffer += chunk;
      const found = buffer.match(/http:\/\/127\.0\.0\.1:(\d+)/);
      if (found) {
        clearTimeout(timer);
        resolvePromise(Number(found[1]));
      }
    });
    child.on('error', (err) => {
      clearTimeout(timer);
      rejectPromise(err);
    });
    child.on('exit', (code) => {
      clearTimeout(timer);
      rejectPromise(new Error('le moteur s’est arrêté (code ' + code + ') : ' + stderr));
    });
  });
  return { child, port };
}

/** Attend que le moteur réponde sur `/health`. */
async function waitForHealth(port) {
  const deadline = Date.now() + 15000;
  while (Date.now() < deadline) {
    try {
      const response = await fetch('http://127.0.0.1:' + port + '/health');
      if (response.ok) {
        return await response.json();
      }
    } catch (err) {
      /* le moteur n'écoute pas encore */
    }
    await new Promise((done) => setTimeout(done, 100));
  }
  throw new Error('le moteur ne répond pas sur /health');
}

/* ------------------------------------------------------------------ */
/* Scénario                                                            */
/* ------------------------------------------------------------------ */

const binary = findEngineBinary();
const vault = mkdtempSync(join(tmpdir(), 'obsidian-rs-shim-'));
mkdirSync(join(vault, 'dossier'), { recursive: true });
writeFileSync(join(vault, 'Note.md'), '# Note\n\ncontenu de la note\n');
writeFileSync(join(vault, 'dossier', 'Autre.md'), '# Autre\n');

// Faux ASAR pour la lecture à l'exécution de la chaîne d'acceptation : on
// n'utilise jamais le vrai fichier d'Obsidian, et aucune de ses chaînes
// n'apparaît ici (littéral synthétique).
// Le contenu reproduit la **forme** que le renderer emploie : le littéral est
// comparé au canal « terms » — c'est cette comparaison qui permet de le
// retrouver sans le recopier.
const acceptationFake = 'a'.repeat(284);
const contenuAppFake =
  'if("' + acceptationFake + '"!==e.ipcRenderer.sendSync("terms"))return window.close();';
const fauxAsarPath = join(tmpdir(), 'obsidian-compat-faux-' + process.pid + '.asar');
writeFileSync(fauxAsarPath, construireFauxAsar(contenuAppFake));


console.log('moteur   : ' + binary);
console.log('vault    : ' + vault);

const engine = await startEngine(binary, vault);
console.log('port     : ' + engine.port);
const health = await waitForHealth(engine.port);
console.log('santé    : ' + JSON.stringify(health));
console.log('');

const shimSource = readFileSync(join(HERE, 'browser-shim.js'), 'utf8');
const sandbox = {
  XMLHttpRequest: FakeXMLHttpRequest,
  console,
  setTimeout,
  clearTimeout,
  TextEncoder,
  TextDecoder
};
sandbox.window = {
  __OBSIDIAN_ENGINE_PORT: engine.port,
  __OBSIDIAN_COMPAT__: { creerSurfaceRemote, lireChaineAcceptation },
  __OBSIDIAN_APP_ASAR__: fauxAsarPath
};
createContext(sandbox);
runInContext(shimSource, sandbox, { filename: 'browser-shim.js' });

const require = sandbox.require;
const fs = require('fs');
const path = require('path');
const crypto = require('crypto');

console.log('fs.readFileSync');
check('le contenu du fichier est renvoyé', () => {
  assertEqual(fs.readFileSync('Note.md', 'utf8'), '# Note\n\ncontenu de la note\n');
});
check('sans encodage, un Buffer est renvoyé', () => {
  const buffer = fs.readFileSync('Note.md');
  assert(buffer && typeof buffer.length === 'number', 'un Buffer est attendu');
  assertEqual(buffer.toString('utf8'), '# Note\n\ncontenu de la note\n');
});
check('un fichier absent lève une erreur not_found', () => {
  let error = null;
  try {
    fs.readFileSync('Absent.md', 'utf8');
  } catch (err) {
    error = err;
  }
  assert(error !== null, 'une erreur est attendue');
  assertEqual(error.code, 'not_found');
});

console.log('fs.existsSync');
check('un fichier présent renvoie true', () => {
  assertEqual(fs.existsSync('Note.md'), true);
});
check('un fichier absent renvoie false', () => {
  assertEqual(fs.existsSync('Absent.md'), false);
});

console.log('path.*');
check('path.join assemble les fragments', () => {
  assertEqual(path.join('dossier', 'Autre.md'), 'dossier/Autre.md');
});
check('path.dirname renvoie le dossier parent', () => {
  assertEqual(path.dirname('dossier/Autre.md'), 'dossier');
});
check('path.basename renvoie le nom du fichier', () => {
  assertEqual(path.basename('dossier/Autre.md'), 'Autre.md');
});
check('path.extname renvoie l’extension', () => {
  assertEqual(path.extname('dossier/Autre.md'), '.md');
});
check('path.resolve est relatif à la racine du vault', () => {
  const resolu = path.resolve('Note.md');
  assert(resolu.startsWith(vault + '/'), 'résolution inattendue : ' + resolu);
});

console.log('confinement des chemins');
check('une remontée hors du vault est refusée', () => {
  let error = null;
  try {
    fs.readFileSync('../../etc/passwd', 'utf8');
  } catch (err) {
    error = err;
  }
  assert(error !== null, 'une erreur est attendue');
  assertEqual(error.code, 'path_refused');
});
check('un chemin absolu est refusé', () => {
  let error = null;
  try {
    fs.readFileSync('/etc/passwd', 'utf8');
  } catch (err) {
    error = err;
  }
  assert(error !== null, 'une erreur est attendue');
  assertEqual(error.code, 'path_refused');
});
check('une écriture hors du vault est refusée', () => {
  let error = null;
  try {
    fs.writeFileSync('../evade.md', 'x', 'utf8');
  } catch (err) {
    error = err;
  }
  assert(error !== null, 'une erreur est attendue');
  assertEqual(error.code, 'path_refused');
  assert(!existsSync(join(vault, '..', 'evade.md')), 'rien ne doit être écrit hors du vault');
});

console.log('écriture, dossiers et métadonnées');
check('fs.writeFileSync écrit puis relu', () => {
  fs.writeFileSync('Ecrit.md', '# Écrit\n', 'utf8');
  assertEqual(fs.readFileSync('Ecrit.md', 'utf8'), '# Écrit\n');
});
check('fs.readdirSync liste le dossier', () => {
  const entrees = fs.readdirSync('dossier');
  assert(entrees.includes('Autre.md'), 'liste inattendue : ' + JSON.stringify(entrees));
});
check('fs.statSync décrit le fichier', () => {
  const stats = fs.statSync('Note.md');
  assert(stats.isFile() === true, 'isFile() doit être vrai');
  assert(stats.isDirectory() === false, 'isDirectory() doit être faux');
  assert(stats.size > 0, 'la taille doit être non nulle');
});
check('fs.mkdirSync puis fs.renameSync puis fs.unlinkSync', () => {
  fs.mkdirSync('cree/sous', { recursive: true });
  assertEqual(fs.existsSync('cree/sous'), true);
  fs.renameSync('Ecrit.md', 'cree/Deplace.md');
  assertEqual(fs.existsSync('Ecrit.md'), false);
  assertEqual(fs.existsSync('cree/Deplace.md'), true);
  fs.unlinkSync('cree/Deplace.md');
  assertEqual(fs.existsSync('cree/Deplace.md'), false);
});

console.log('crypto.randomUUID');
check('un UUID version 4 est renvoyé', () => {
  const uuid = crypto.randomUUID();
  assert(
    /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/.test(uuid),
    'UUID invalide : ' + uuid
  );
});

console.log('electron.ipcRenderer');
check('sendSync("vault") renvoie la racine du vault', () => {
  const vaultInfo = sandbox.electron.ipcRenderer.sendSync('vault');
  assert(vaultInfo && typeof vaultInfo.path === 'string', 'chemin manquant');
  assert(vaultInfo.path.startsWith(vault), 'racine inattendue : ' + vaultInfo.path);
});
check('sendSync("vault-list") renvoie la liste des vaults', () => {
  const liste = sandbox.electron.ipcRenderer.sendSync('vault-list');
  assert(Array.isArray(liste.vaults) && liste.vaults.length === 1, 'liste inattendue');
});
check('un canal inconnu renvoie undefined', () => {
  assertEqual(sandbox.electron.ipcRenderer.sendSync('canal-inconnu'), undefined);
});

console.log('chemins absolus sous la racine');
check('un chemin absolu sous la racine est traduit en relatif', () => {
  assertEqual(fs.readFileSync(join(vault, 'Note.md'), 'utf8'), '# Note\n\ncontenu de la note\n');
});
check('fs.existsSync(racine) renvoie true', () => {
  assertEqual(fs.existsSync(vault), true);
});
check('une écriture par chemin absolu sous la racine fonctionne', () => {
  const cible = join(vault, 'Absolue.md');
  fs.writeFileSync(cible, '# Absolue\n', 'utf8');
  assertEqual(fs.readFileSync(cible, 'utf8'), '# Absolue\n');
});
check('un chemin absolu hors de la racine est refusé', () => {
  let error = null;
  try {
    fs.readFileSync(join(vault, '..', 'hors.md'), 'utf8');
  } catch (err) {
    error = err;
  }
  assert(error !== null, 'une erreur est attendue');
  assertEqual(error.code, 'path_refused');
});

console.log('canal vault : identifiant stable');
check('sendSync("vault") renvoie un id stable et non vide', () => {
  const a = sandbox.electron.ipcRenderer.sendSync('vault');
  const b = sandbox.electron.ipcRenderer.sendSync('vault');
  assert(typeof a.id === 'string' && a.id.length > 0, 'id manquant');
  assertEqual(a.id, b.id);
  assertEqual(a.path, vault);
});

console.log('module url');
const urlMod = require('url');
check('url.parse décompose une URL', () => {
  const u = urlMod.parse('https://exemple.com/a/b?q=1#fragment');
  assertEqual(u.protocol, 'https:');
  assertEqual(u.hostname, 'exemple.com');
  assertEqual(u.pathname, '/a/b');
  assertEqual(u.search, '?q=1');
  assertEqual(u.hash, '#fragment');
});
check('url.fileURLToPath convertit une URL file://', () => {
  assertEqual(urlMod.fileURLToPath('file:///tmp/Note%20A.md'), '/tmp/Note A.md');
});
check('url.pathToFileURL construit une URL file://', () => {
  const fichier = urlMod.pathToFileURL('/tmp/Note.md');
  assertEqual(fichier.href, 'file:///tmp/Note.md');
  assertEqual(String(fichier), 'file:///tmp/Note.md');
});

console.log('os.version');
check('os.version() et os.release() renvoient la version du noyau', () => {
  const osMod = require('os');
  const version = osMod.version();
  const release = osMod.release();
  assert(typeof version === 'string' && version.length > 0, 'version vide');
  assert(typeof release === 'string' && release.length > 0, 'release vide');
  assertEqual(osMod.platform(), 'linux');
});

console.log('surface electron.remote');
check('remote.app.getPreferredSystemLanguages renvoie des langues', () => {
  const langues = sandbox.electron.remote.app.getPreferredSystemLanguages();
  assert(Array.isArray(langues) && langues.length > 0, 'aucune langue');
});
check('remote.getCurrentWindow expose l’état fenêtré sous forme de méthodes', () => {
  const fenetre = sandbox.electron.remote.getCurrentWindow();
  for (const membre of ['minimizable', 'maximizable', 'closable', 'isFullScreen', 'isMaximized', 'isFocused']) {
    assert(typeof fenetre[membre] === 'function', membre + ' doit être une méthode');
  }
  assert(typeof fenetre.webContents.getZoomFactor === 'function', 'getZoomFactor manquant');
  assertEqual(fenetre.webContents.getZoomFactor(), 1);
});
check('nativeTheme.removeAllListeners est chaînable', () => {
  const theme = sandbox.electron.nativeTheme;
  const retour = theme.removeAllListeners('updated');
  assert(retour === theme, 'removeAllListeners doit retourner l’émetteur');
  assert(typeof theme.on === 'function', 'on manquant');
});

console.log('chaîne d’acceptation lue à l’exécution');
check('lireChaineAcceptation lit la constante dans un faux ASAR', () => {
  assertEqual(lireChaineAcceptation(fauxAsarPath), acceptationFake);
});
check('lireChaineAcceptation renvoie null si l’application est absente', () => {
  const absent = join(vault, 'absence.asar');
  assertEqual(lireChaineAcceptation(absent), null);
});
check('sendSync("terms") renvoie la chaîne lue depuis l’ASAR', () => {
  assertEqual(sandbox.electron.ipcRenderer.sendSync('terms'), acceptationFake);
});
check('sendSync("terms") renvoie null (jamais vide) si aucune application', () => {
  sandbox.window.__OBSIDIAN_APP_ASAR__ = undefined;
  const termes = sandbox.electron.ipcRenderer.sendSync('terms');
  assert(termes === null || termes === undefined, 'terms ne doit pas être une chaîne vide');
});

console.log('transport');
check('les requêtes sont bien synchrones', () => {
  assert(sentRequests.length > 0, 'aucune requête envoyée');
  const asynchrones = sentRequests.filter((request) => request.async !== false);
  assert(
    asynchrones.length === 0,
    'une requête asynchrone a été envoyée : ' + JSON.stringify(asynchrones[0])
  );
});
check('les requêtes sont en POST sur /sync', () => {
  for (const request of sentRequests) {
    assertEqual(request.method, 'POST');
    assert(request.url.endsWith('/sync'), 'URL inattendue : ' + request.url);
  }
});
check('le Content-Type est « safelisted » (pas de préflight)', () => {
  for (const request of sentRequests) {
    assert(
      request.contentType.indexOf('text/plain') === 0,
      'Content-Type inattendu : ' + request.contentType
    );
  }
});
check('les opérations du moteur sont bien celles attendues', () => {
  const ops = new Set(sentRequests.map((request) => request.op));
  for (const expected of ['fs.readFileSync', 'fs.existsSync', 'path.join', 'fs.writeFileSync']) {
    assert(ops.has(expected), 'opération jamais vue : ' + expected);
  }
});

/* ------------------------------------------------------------------ */
/* Mesure du coût du pont synchrone                                   */
/* ------------------------------------------------------------------ */

const SAMPLES = 200;
const debut = process.hrtime.bigint();
for (let i = 0; i < SAMPLES; i += 1) {
  fs.readFileSync('Note.md', 'utf8');
}
const dureeMs = Number(process.hrtime.bigint() - debut) / 1e6;
console.log('');
console.log(
  'performance : ' +
    SAMPLES +
    ' appels synchrones en ' +
    dureeMs.toFixed(1) +
    ' ms (' +
    (dureeMs / SAMPLES).toFixed(3) +
    ' ms par aller-retour, moteur local)'
);

/* ------------------------------------------------------------------ */
/* Nettoyage                                                           */
/* ------------------------------------------------------------------ */

engine.child.kill('SIGTERM');
await worker.terminate();
rmSync(vault, { recursive: true, force: true });
rmSync(fauxAsarPath, { force: true });

console.log('');
console.log('résultat : ' + passed + ' ok, ' + failed + ' échec(s)');
process.exit(failed === 0 ? 0 : 1);
