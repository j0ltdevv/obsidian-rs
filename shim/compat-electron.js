/**
 * Compat Electron et lecture de la chaîne d'acceptation.
 *
 * Ce module est consommé par `browser-shim.js` via
 * `window.__OBSIDIAN_COMPAT__`. Il expose :
 *
 *   - `creerSurfaceRemote(config)` : construit la surface `electron.remote`
 *     minimale requise par le renderer d'Obsidian (`getCurrentWindow`,
 *     `app.getPreferredSystemLanguages`, `nativeTheme.removeAllListeners`
 *     chaînable…) ;
 *
 *   - `lireChaineAcceptation(cheminApp, lecteurFichier?)` : ouvre l'ASAR de
 *     l'application Obsidian installée, en extrait `app.js` et y localise la
 *     chaîne d'acceptation de 284 caractères. Rien n'est recopié ici : la
 *     chaîne est lue à chaque exécution. En cas d'échec (application absente,
 *     ASAR invalide, constante introuvable), elle renvoie `null` et journalise
 *     un message explicite — jamais une chaîne vide.
 *
 *   - `parseAsar(octets)` : parse l'en-tête ASAR (exposé pour les tests).
 *
 * Le format ASAR (Electron) est un en-tête pickle (deux entiers 32 bits
 * little-endian : taille du pickle, taille de la chaîne), suivi d'un pickle
 * contenant la chaîne JSON de l'en-tête, puis des données des fichiers. Les
 * décalages des fichiers sont relatifs au début des données (`8 + taillePickle`).
 */

/* ------------------------------------------------------------------ */
/* Journalisation explicite                                            */
/* ------------------------------------------------------------------ */

function journaliser(message) {
  try {
    if (typeof console !== 'undefined' && console.log) {
      console.log('[compat] ' + message);
    }
  } catch (e) {
    /* ignore */
  }
}

/* ------------------------------------------------------------------ */
/* Surface `electron.remote`                                           */
/* ------------------------------------------------------------------ */

export function creerSurfaceRemote(config) {
  const c = config || {};
  const locales =
    Array.isArray(c.locales) && c.locales.length ? c.locales.slice() : ['fr-FR'];

  const app = Object.assign({}, c.app || {});
  app.getPreferredSystemLanguages = function () {
    return locales.slice();
  };
  if (typeof app.getLocale !== 'function') {
    app.getLocale = function () {
      return locales[0] || 'fr-FR';
    };
  }

  const nativeTheme = Object.assign({}, c.nativeTheme || {});
  if (typeof nativeTheme.removeAllListeners !== 'function') {
    nativeTheme.removeAllListeners = function () {
      return nativeTheme;
    };
  }

  const getCurrentWindow = function () {
    return {
      minimizable: function () {
        return true;
      },
      maximizable: function () {
        return true;
      },
      closable: function () {
        return true;
      },
      isFullScreen: function () {
        return false;
      },
      isMaximized: function () {
        return false;
      },
      isFocused: function () {
        return true;
      },
      webContents: {
        getZoomFactor: function () {
          return 1;
        }
      },
      getBounds: function () {
        return { x: 0, y: 0, width: 1280, height: 800 };
      },
      setBounds: function () {},
      center: function () {}
    };
  };

  return {
    app: app,
    clipboard: c.clipboard || {},
    dialog: c.dialog || {},
    shell: c.shell || {},
    nativeTheme: nativeTheme,
    webFrame: c.webFrame || {},
    screen: c.screen || {},
    Menu: c.Menu || {},
    safeStorage: c.safeStorage || {},
    systemPreferences: c.systemPreferences || {},
    getCurrentWebContents: function () {
      return { session: { fromPartition: function () { return {}; } } };
    },
    getCurrentWindow: getCurrentWindow
  };
}

/* ------------------------------------------------------------------ */
/* ASAR                                                               */
/* ------------------------------------------------------------------ */

function decodeUtf8(octets) {
  if (typeof TextDecoder !== 'undefined') {
    return new TextDecoder('utf-8').decode(octets);
  }
  let texte = '';
  for (let i = 0; i < octets.length; i += 1) {
    texte += String.fromCharCode(octets[i]);
  }
  try {
    return decodeURIComponent(escape(texte));
  } catch (e) {
    return texte;
  }
}

/**
 * Parse l'en-tête ASAR à partir des octets du fichier.
 * Retourne `{ header, dataStart }` ou `null` si l'en-tête est invalide.
 *
 * Format (Electron, `@electron/asar`) :
 *   - un « size pickle » de 8 octets : `[payloadSize(4)] [tailleHeaderBuf(4)]`
 *     où `tailleHeaderBuf` est la longueur du pickle d'en-tête qui suit ;
 *   - le pickle d'en-tête (taille `tailleHeaderBuf`) : `[payloadSize(4)]
 *     [tailleChaine(4)] [JSON]`, `tailleChaine` étant la longueur en octets
 *     de la chaîne JSON décrivant l'arborescence ;
 *   - les données des fichiers commencent à `8 + tailleHeaderBuf` ; le
 *     décalage de chaque entrée y est relatif.
 */
export function parseAsar(octets) {
  const vue = new DataView(octets.buffer, octets.byteOffset, octets.byteLength);
  if (octets.byteLength < 8 + 8 + 8) {
    return null;
  }
  // Size pickle : sa charge utile est exactement un entier 32 bits (la
  // longueur du pickle d'en-tête).
  const taillePickleTaille = vue.getUint32(0, true);
  const tailleHeaderBuf = vue.getUint32(4, true);
  if (taillePickleTaille !== 4) {
    return null;
  }
  const debutHeader = 8;
  if (debutHeader + tailleHeaderBuf > octets.byteLength) {
    return null;
  }
  const headerVue = new DataView(
    octets.buffer,
    octets.byteOffset + debutHeader,
    tailleHeaderBuf
  );
  const payloadSizeHeader = headerVue.getUint32(0, true);
  const tailleChaine = headerVue.getUint32(4, true);
  if (tailleChaine <= 0 || debutHeader + 8 + tailleChaine > octets.byteLength) {
    return null;
  }
  if (payloadSizeHeader < 4 + tailleChaine) {
    return null;
  }
  const jsonBrut = new Uint8Array(
    octets.buffer,
    octets.byteOffset + debutHeader + 8,
    tailleChaine
  );
  let header;
  try {
    header = JSON.parse(decodeUtf8(jsonBrut));
  } catch (e) {
    return null;
  }
  if (!header || typeof header !== 'object' || !header.files) {
    return null;
  }
  return { header: header, dataStart: debutHeader + tailleHeaderBuf };
}

function trouverFichier(fichiers, nom) {
  const direct = fichiers[nom];
  if (direct && typeof direct.offset !== 'undefined') {
    return direct;
  }
  const cles = Object.keys(fichiers);
  for (let i = 0; i < cles.length; i += 1) {
    const valeur = fichiers[cles[i]];
    if (valeur && typeof valeur === 'object' && valeur.files) {
      const trouve = trouverFichier(valeur.files, nom);
      if (trouve) {
        return trouve;
      }
    }
  }
  return null;
}

function extraireFichier(octets, dataStart, entree) {
  const offset = parseInt(String(entree.offset), 10);
  const taille = entree.size;
  if (!Number.isFinite(offset) || offset < 0) {
    return null;
  }
  const debut = dataStart + offset;
  if (debut < dataStart || debut + taille > octets.byteLength) {
    return null;
  }
  return new Uint8Array(octets.buffer, octets.byteOffset + debut, taille);
}

/* ------------------------------------------------------------------ */
/* Lecture de la chaîne d'acceptation                                 */
/* ------------------------------------------------------------------ */

function lireOctets(chemin, lecteur) {
  if (typeof lecteur === 'function') {
    return lecteur(chemin);
  }
  if (
    typeof globalThis !== 'undefined' &&
    typeof globalThis.__OBSIDIAN_LIRE_FICHIER__ === 'function'
  ) {
    return globalThis.__OBSIDIAN_LIRE_FICHIER__(chemin);
  }
  if (
    typeof process !== 'undefined' &&
    process.versions &&
    typeof process.getBuiltinModule === 'function'
  ) {
    return process.getBuiltinModule('fs').readFileSync(chemin);
  }
  return null;
}

/**
 * Ouvre l'ASAR de l'application Obsidian installée, en extrait `app.js` et y
 * localise la chaîne d'acceptation (littéral chaîne de 284 caractères).
 * Renvoie la chaîne, ou `null` si l'application est absente, l'ASAR
 * invalide, ou la constante introuvable — en journalisant un message explicite.
 */
export function lireChaineAcceptation(cheminApp, lecteurFichier) {
  let octets;
  try {
    octets = lireOctets(cheminApp, lecteurFichier);
  } catch (e) {
    journaliser('lecture impossible de l\'application : ' + cheminApp);
    return null;
  }
  if (!octets) {
    journaliser('application absente ou illisible : ' + cheminApp);
    return null;
  }
  const asar = parseAsar(octets);
  if (!asar) {
    journaliser('en-tête ASAR invalide : ' + cheminApp);
    return null;
  }
  const appJs = trouverFichier(asar.header.files, 'app.js');
  if (!appJs) {
    journaliser('app.js introuvable dans l\'ASAR : ' + cheminApp);
    return null;
  }
  const contenu = extraireFichier(octets, asar.dataStart, appJs);
  if (contenu === null) {
    journaliser('extraction de app.js impossible dans : ' + cheminApp);
    return null;
  }
  const texte =
    typeof contenu === 'string' ? contenu : decodeUtf8(contenu);
  // On localise la constante par sa seule longueur (284 caractères) : aucun
  // texte d'Obsidian n'est recopié dans ce dépôt.
  const match = /"((?:[^"\\]|\\.){284})"/.exec(texte);
  if (!match) {
    journaliser('chaîne d\'acceptation (284 caractères) introuvable dans app.js');
    return null;
  }
  return match[1];
}
