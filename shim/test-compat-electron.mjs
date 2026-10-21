/*
 * Test du lanceur du mode « compat » (`shim/electron-main-compat.js`).
 *
 * Lance Xvfb + Electron + le lanceur sur le vault de test et l'ASAR de la
 * boîte, attend l'apparition de la fenêtre réelle d'Obsidian, puis vérifie
 * via CDP : une fenêtre existe, son titre contient « Obsidian »,
 * `.app-container` et `.workspace` sont présents dans le DOM, et aucune
 * exception n'a été journalisée par le renderer. Capture d'écran de la
 * fenêtre (`Page.captureScreenshot`) et vérification que les données
 * affichées viennent bien du moteur Rust (`GET /files`).
 *
 * Échec = message réel (sortie des processus, exceptions, etc.).
 *
 * Configuration (variables d'environnement) :
 *   OBSIDIAN_COMPAT_VAULT      racine du vault de test
 *   OBSIDIAN_COMPAT_ASAR       dossier d'ASAR extrait
 *   OBSIDIAN_COMPAT_ENGINE     binaire du moteur Rust
 *   OBSIDIAN_COMPAT_ELECTRON   binaire Electron (si absent, recherche)
 *   XVFB_DISPLAY               écran Xvfb (défaut :91)
 *   CDP_PORT                   port de débogage CDP (défaut 9244)
 *   STATIC_PORT                port serveur statique (défaut 8823)
 *   ENGINE_PORT                port du moteur (défaut 27124)
 *   CAPTURE_PATH               chemin de la capture (défaut demonstrations/obsidian-rea/mode-compat-capture.png)
 */
import { spawn } from 'node:child_process';
import http from 'node:http';
import path from 'node:path';
import fs from 'node:fs';
import { fileURLToPath } from 'node:url';
import { setTimeout as attendre } from 'node:timers/promises';

const ICI = path.dirname(fileURLToPath(import.meta.url));
const ENV = process.env;

const DEFAUT_CAPTURE = path.join(
  ICI,
  '..',
  'demonstrations',
  'obsidian-rea',
  'mode-compat-capture.png'
);

const CONFIG = {
  vault: ENV.OBSIDIAN_COMPAT_VAULT || '',
  asarDir: ENV.OBSIDIAN_COMPAT_ASAR || '',
  engineBin: ENV.OBSIDIAN_COMPAT_ENGINE || '',
  electronBin: ENV.OBSIDIAN_COMPAT_ELECTRON || '',
  display: ENV.XVFB_DISPLAY || ':91',
  cdpPort: Number(ENV.CDP_PORT || 9244),
  staticPort: Number(ENV.STATIC_PORT || 8823),
  enginePort: Number(ENV.ENGINE_PORT || 27124),
  capturePath: ENV.CAPTURE_PATH || DEFAUT_CAPTURE,
};

function chercherElectron() {
  const candidats = [
    CONFIG.electronBin,
    path.join(ICI, '..', 'tools', 'electron', 'node_modules', '.bin', 'electron'),
    '/data/home/obsidian-rea/tools/electron/node_modules/.bin/electron',
  ].filter(Boolean);
  for (const c of candidats) {
    if (fs.existsSync(c)) return c;
  }
  return 'electron'; // dernier recours : PATH
}

function checkPreRequis() {
  const manque = [];
  if (!CONFIG.vault || !fs.existsSync(CONFIG.vault)) {
    manque.push('OBSIDIAN_COMPAT_VAULT : ' + CONFIG.vault);
  }
  if (!CONFIG.asarDir || !fs.existsSync(CONFIG.asarDir)) {
    manque.push('OBSIDIAN_COMPAT_ASAR : ' + CONFIG.asarDir);
  }
  if (!CONFIG.engineBin || !fs.existsSync(CONFIG.engineBin)) {
    manque.push('OBSIDIAN_COMPAT_ENGINE : ' + CONFIG.engineBin);
  }
  return manque;
}

/* -------------------------------------------------------------------- */
/* CDP minimal (HTTP /json/list + WebSocket)                             */
/* -------------------------------------------------------------------- */

function listerCibles(port) {
  return new Promise((resolve, reject) => {
    const req = http.get({ host: '127.0.0.1', port, path: '/json/list', timeout: 2000 }, (res) => {
      let corps = '';
      res.on('data', (d) => { corps += d; });
      res.on('end', () => {
        try { resolve(JSON.parse(corps)); } catch (e) { reject(e); }
      });
    });
    req.on('error', reject);
    req.on('timeout', () => { req.destroy(); reject(new Error('timeout CDP')); });
  });
}

class CdpClient {
  constructor(url) {
    this.url = url;
    this.ws = null;
    this.pending = new Map();
    this.id = 0;
  }
  async ouvrir() {
    this.ws = new WebSocket(this.url);
    await new Promise((resolve, reject) => {
      this.ws.addEventListener('open', resolve, { once: true });
      this.ws.addEventListener('error', () => reject(new Error('WebSocket CDP refusé')), { once: true });
    });
    this.ws.addEventListener('message', (ev) => {
      let message;
      try { message = JSON.parse(ev.data); } catch (e) { return; }
      if (message.id && this.pending.has(message.id)) {
        const p = this.pending.get(message.id);
        this.pending.delete(message.id);
        if (message.error) p.reject(new Error(message.error.message));
        else p.resolve(message.result);
      }
    });
  }
  envoyer(methode, params) {
    return new Promise((resolve, reject) => {
      const id = ++this.id;
      this.pending.set(id, { resolve, reject });
      this.ws.send(JSON.stringify({ id, method: methode, params: params || {} }));
    });
  }
  fermer() {
    try { if (this.ws) this.ws.close(); } catch (e) { /* ignore */ }
  }
}

async function evaluerDom(cdp) {
  const r = await cdp.envoyer('Runtime.evaluate', {
    expression:
      '({title: document.title, appContainer: !!document.querySelector(".app-container"), workspace: !!document.querySelector(".workspace"), fileTitles: Array.from(document.querySelectorAll(".nav-file-title")).map(e => (e.textContent || "").trim())})',
    returnByValue: true,
  });
  if (r.exceptionDetails) {
    throw new Error('évaluation CDP en échec : ' + JSON.stringify(r.exceptionDetails));
  }
  return r.result.value;
}

async function capturerEcran(cdp) {
  const r = await cdp.envoyer('Page.captureScreenshot', { format: 'png' });
  if (!r || !r.data) {
    throw new Error('capture d\'écran CDP vide : ' + JSON.stringify(r));
  }
  fs.mkdirSync(path.dirname(CONFIG.capturePath), { recursive: true });
  fs.writeFileSync(CONFIG.capturePath, Buffer.from(r.data, 'base64'));
  return CONFIG.capturePath;
}

function requeteJson(port, chemin) {
  return new Promise((resolve, reject) => {
    const req = http.get({ host: '127.0.0.1', port, path: chemin, timeout: 4000 }, (res) => {
      let corps = '';
      res.on('data', (d) => { corps += d; });
      res.on('end', () => {
        try { resolve(JSON.parse(corps)); } catch (e) { reject(e); }
      });
    });
    req.on('error', reject);
    req.on('timeout', () => { req.destroy(); reject(new Error('timeout ' + chemin)); });
  });
}

/* -------------------------------------------------------------------- */
/* Processus                                                             */
/* -------------------------------------------------------------------- */

async function demarrerXvfb() {
  const xvfb = spawn('Xvfb', [CONFIG.display, '-screen', '0', '1400x900x24'], {
    stdio: ['ignore', 'pipe', 'pipe'],
  });
  const sortie = [];
  xvfb.stderr.on('data', (d) => sortie.push(d.toString()));
  await attendre(700);
  if (xvfb.exitCode !== null) {
    throw new Error('Xvfb a quitté immédiatement : ' + sortie.join(''));
  }
  return xvfb;
}

function demarrerElectron(launcher) {
  const args = [
    launcher,
    '--vault', CONFIG.vault,
    '--asar', CONFIG.asarDir,
    '--engine-bin', CONFIG.engineBin,
    '--engine-port', String(CONFIG.enginePort),
    '--static-port', String(CONFIG.staticPort),
    '--no-sandbox',
    '--disable-gpu',
    '--remote-debugging-port=' + CONFIG.cdpPort,
  ];
  const enfant = spawn(CONFIG.electronBin, args, {
    env: { ...process.env, DISPLAY: CONFIG.display },
    stdio: ['ignore', 'pipe', 'pipe'],
  });
  const stdout = [];
  const stderr = [];
  enfant.stdout.on('data', (d) => { stdout.push(d.toString()); });
  enfant.stderr.on('data', (d) => { stderr.push(d.toString()); });
  return { enfant, stdout, stderr };
}

function erreursRenderer(stderr) {
  return stderr.join('').split('\n').filter((ligne) =>
    /\[renderer\]\s+error:/i.test(ligne) ||
    /\[renderer\]\s+.*\bUncaught\b/.test(ligne) ||
    /\[renderer\]\s+.*\b(TypeError|ReferenceError|SyntaxError|RangeError):/.test(ligne)
  );
}

/* -------------------------------------------------------------------- */
/* Déroulé                                                               */
/* -------------------------------------------------------------------- */

function usage() {
  console.error('Utilisation : node shim/test-compat-electron.mjs');
  console.error('Variables requises : OBSIDIAN_COMPAT_VAULT, OBSIDIAN_COMPAT_ASAR, OBSIDIAN_COMPAT_ENGINE');
  console.error('Options : OBSIDIAN_COMPAT_ELECTRON, XVFB_DISPLAY, CDP_PORT, STATIC_PORT, ENGINE_PORT, CAPTURE_PATH');
}

const manque = checkPreRequis();
if (manque.length) {
  console.error('[test] prérequis manquants :');
  for (const m of manque) console.error('  - ' + m);
  usage();
  process.exit(2);
}

CONFIG.electronBin = chercherElectron();
console.log('[test] electron  :', CONFIG.electronBin);
console.log('[test] vault     :', CONFIG.vault);
console.log('[test] asar     :', CONFIG.asarDir);
console.log('[test] moteur   :', CONFIG.engineBin);
console.log('[test] affichage:', CONFIG.display);
console.log('[test] capture   :', CONFIG.capturePath);

let xvfb;
let enfant;
let cdp;

try {
  xvfb = await demarrerXvfb();

  const launcher = path.join(ICI, 'electron-main-compat.js');
  const lance = demarrerElectron(launcher);
  enfant = lance.enfant;
  const { stdout, stderr } = lance;

  // 1. Attendre une cible page dont le titre contient « Obsidian ».
  let cible = null;
  const fin = Date.now() + 45000;
  let dernierErreur = '';
  while (Date.now() < fin) {
    if (enfant.exitCode !== null) {
      throw new Error(
        'Electron a quitté tôt (code=' + enfant.exitCode + ').\n' +
        'STDOUT:\n' + stdout.join('') + '\nSTDERR:\n' + stderr.join('')
      );
    }
    try {
      const cibles = await listerCibles(CONFIG.cdpPort);
      cible = (cibles || []).find((c) => c.type === 'page' && /Obsidian/.test(c.title || ''));
      if (cible) break;
    } catch (e) {
      dernierErreur = e.message;
    }
    await attendre(500);
  }

  if (!cible) {
    throw new Error(
      'Aucune fenêtre Obsidian détectée via CDP (port ' + CONFIG.cdpPort + '). ' +
      'Dernière erreur CDP : ' + dernierErreur + '.\n' +
      'STDOUT:\n' + stdout.join('') + '\nSTDERR:\n' + stderr.join('')
    );
  }
  console.log('[test] fenêtre détectée :', cible.title);

  // 2. Vérifier le DOM (.app-container et .workspace) — on réessaie quelques secondes.
  cdp = new CdpClient(cible.webSocketDebuggerUrl);

  let etat = null;
  const finDom = Date.now() + 20000;
  while (Date.now() < finDom) {
    etat = await evaluerDom(cdp);
    if (etat.appContainer && etat.workspace) break;
    await attendre(800);
  }

  console.log('[test] DOM mesuré :', JSON.stringify(etat));

  // 3. Zéro exception journalisée par le renderer.
  const erreurs = erreursRenderer(stderr);
  if (erreurs.length) {
    const extrait = erreurs.slice(0, 10).join('\n  ');
    throw new Error('Exceptions renderer détectées (' + erreurs.length + ') :\n  ' + extrait);
  }

  const ok =
    etat &&
    /Obsidian/.test(etat.title) &&
    etat.appContainer &&
    etat.workspace;

  if (!ok) {
    throw new Error('Critères non satisfaits : ' + JSON.stringify(etat));
  }

  // 4. Capture d'écran de la fenêtre (via CDP).
  const capture = await capturerEcran(cdp);
  console.log('[test] capture d\'écran :', capture);

  // 5. Les données affichées viennent du moteur Rust : interroge /files
  //    et compare avec les titres de fichiers listés par le renderer.
  let moteur = null;
  let comparaison = 'non vérifiée';
  try {
    moteur = await requeteJson(CONFIG.enginePort, '/files');
    const fichiersMoteur = new Set(
      (moteur.files || []).map((f) => path.basename(f.path, path.extname(f.path)))
    );
    const titres = new Set(
      (etat.fileTitles || []).map((t) => path.basename(String(t), path.extname(String(t))))
    );
    let intersection = 0;
    for (const t of titres) {
      if (fichiersMoteur.has(t)) intersection += 1;
    }
    if (intersection > 0) {
      comparaison = 'ok (' + intersection + ' fichier(s) commun(s))';
    } else {
      comparaison =
        'aucun titre de fichier ne correspond aux fichiers du moteur — ' +
        'moteur.files=' + (moteur.files || []).length +
        ', titres renderer=' + titres.size;
    }
    console.log('[test] comparaison moteur/renderer :', comparaison);
  } catch (e) {
    throw new Error('Impossible d\'interroger le moteur /files : ' + e.message);
  }
  if (/^aucun/.test(comparaison)) {
    throw new Error('Le renderer n\'affiche pas les données du moteur : ' + comparaison);
  }

  console.log('[test] RÉUSSI : fenêtre + DOM + zéro exception + capture + données moteur.');
  process.exitCode = 0;
} catch (err) {
  console.error('[test] ÉCHEC :', err.message);
  process.exitCode = 1;
} finally {
  if (cdp) cdp.fermer();
  if (enfant && enfant.exitCode === null) {
    try { enfant.kill('SIGTERM'); } catch (e) { /* ignore */ }
    await attendre(800);
  }
  if (xvfb && xvfb.exitCode === null) {
    try { xvfb.kill('SIGTERM'); } catch (e) { /* ignore */ }
  }
}
