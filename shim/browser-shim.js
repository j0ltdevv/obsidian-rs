/*!
 * Shim navigateur pour le moteur obsidian-rs.
 *
 * Le renderer d'Obsidian (ASAR 1.14.4, `app.js`) suppose un `nodeIntegration`
 * actif : il appelle directement `window.require(...)`, `window.process` et
 * `window.electron` (aucun `preload`, aucun `contextBridge`). Ce script
 * installe ces trois globales **avant** le chargement de `app.js`.
 *
 * Transport : le renderer appelle `ipcRenderer.sendSync` (31 canaux) et des
 * fonctions `fs` synchrones. L'IPC de Tauri est asynchrone, donc la réponse
 * passe par un `XMLHttpRequest` **synchrone** vers `POST /sync` du moteur
 * Rust (`http://127.0.0.1:<port>/sync`).
 *
 * Configuration (définie par la page hôte AVANT le chargement du shim) :
 *   - `window.__OBSIDIAN_ENGINE_PORT`   : port du moteur (défaut 27123) ;
 *   - `window.__OBSIDIAN_ENGINE_ORIGIN` : origine complète (défaut
 *     `http://127.0.0.1:<port>`).
 *
 * Les chemins sont relatifs à la racine du vault : le moteur refuse toute
 * remontée (`..`), tout chemin absolu et tout lien symbolique sortant.
 */
(function () {
  'use strict';

  var globalScope =
    typeof globalThis !== 'undefined'
      ? globalThis
      : typeof window !== 'undefined'
        ? window
        : this;
  var host =
    typeof window !== 'undefined' && window ? window : globalScope;

  var PORT = Number(host.__OBSIDIAN_ENGINE_PORT || 27123) || 27123;
  var ORIGIN =
    host.__OBSIDIAN_ENGINE_ORIGIN || 'http://127.0.0.1:' + PORT;
  var ENDPOINT = ORIGIN.replace(/\/+$/, '') + '/sync';

  var ENGINE_VERSION = '1.14.4';
  var RESOURCES_PATH = '/usr/lib/obsidian';

  /* ------------------------------------------------------------------ */
  /* Transport synchrone                                                */
  /* ------------------------------------------------------------------ */

  /**
   * Erreur renvoyée par le moteur : porte le code machine (`not_found`,
   * `path_refused`, …) pour que l'appelant puisse distinguer une absence
   * d'un refus.
   */
  function engineError(message, code) {
    var error = new Error(message);
    error.code = code || 'unknown';
    return error;
  }

  /**
   * Appel synchrone d'une opération du moteur.
   *
   * Le `Content-Type` est `text/plain` : c'est un type CORS « safelisted »,
   * donc la requête n'a pas besoin d'un préflight — impossible à réaliser en
   * XHR synchrone, qui est bloqué par le navigateur.
   */
  function callSync(op, args) {
    var xhr = new XMLHttpRequest();
    try {
      xhr.open('POST', ENDPOINT, false);
    } catch (err) {
      throw engineError(
        'moteur obsidian-rs injoignable (' + ENDPOINT + ') : ' + err.message,
        'unreachable'
      );
    }
    xhr.setRequestHeader('Content-Type', 'text/plain;charset=UTF-8');
    try {
      xhr.send(JSON.stringify({ op: op, args: args || [] }));
    } catch (err) {
      // Moteur arrêté ou port fermé : l'erreur réseau du navigateur est
      // convertie en message explicite.
      throw engineError(
        'moteur obsidian-rs injoignable (' + ENDPOINT + ') : ' + err.message,
        'unreachable'
      );
    }
    var payload;
    try {
      payload = JSON.parse(xhr.responseText);
    } catch (err) {
      throw engineError(
        'réponse illisible du moteur (HTTP ' + xhr.status + ') : ' + xhr.responseText,
        'protocol'
      );
    }
    if (!payload || payload.ok !== true) {
      throw engineError(
        (payload && payload.error) || 'échec de l’opération ' + op,
        (payload && payload.code) || 'unknown'
      );
    }
    return payload.value;
  }

  /* ------------------------------------------------------------------ */
  /* Buffer minimal (le renderer utilise `Buffer.from`)                */
  /* ------------------------------------------------------------------ */

  function createBuffer() {
    if (typeof Uint8Array === 'undefined') {
      return null;
    }
    var Encoder =
      typeof TextEncoder !== 'undefined' ? TextEncoder : null;
    var Decoder =
      typeof TextDecoder !== 'undefined' ? TextDecoder : null;

    function encode(text, encoding) {
      var name = String(encoding || 'utf8').toLowerCase();
      if (name === 'utf8' || name === 'utf-8') {
        if (Encoder) {
          return new Encoder().encode(text);
        }
        var utf = unescape(encodeURIComponent(text));
        var out = new Uint8Array(utf.length);
        for (var i = 0; i < utf.length; i += 1) {
          out[i] = utf.charCodeAt(i);
        }
        return out;
      }
      if (name === 'latin1' || name === 'binary' || name === 'ascii') {
        var bytes = new Uint8Array(text.length);
        for (var j = 0; j < text.length; j += 1) {
          bytes[j] = text.charCodeAt(j) & 0xff;
        }
        return bytes;
      }
      if (name === 'hex') {
        var hex = text.replace(/[^0-9a-fA-F]/g, '');
        var hexBytes = new Uint8Array(Math.floor(hex.length / 2));
        for (var k = 0; k < hexBytes.length; k += 1) {
          hexBytes[k] = parseInt(hex.substr(k * 2, 2), 16);
        }
        return hexBytes;
      }
      if (name === 'base64') {
        if (typeof atob === 'undefined') {
          throw engineError('encodage base64 non supporté', 'unsupported');
        }
        var binary = atob(text);
        var base64Bytes = new Uint8Array(binary.length);
        for (var m = 0; m < binary.length; m += 1) {
          base64Bytes[m] = binary.charCodeAt(m);
        }
        return base64Bytes;
      }
      throw engineError('encodage non supporté : ' + encoding, 'unsupported');
    }

    function decode(bytes, encoding) {
      var name = String(encoding || 'utf8').toLowerCase();
      if (name === 'utf8' || name === 'utf-8') {
        if (Decoder) {
          return new Decoder().decode(bytes);
        }
        var text = '';
        for (var i = 0; i < bytes.length; i += 1) {
          text += String.fromCharCode(bytes[i]);
        }
        return decodeURIComponent(escape(text));
      }
      if (name === 'latin1' || name === 'binary' || name === 'ascii') {
        var latin = '';
        for (var j = 0; j < bytes.length; j += 1) {
          latin += String.fromCharCode(bytes[j]);
        }
        return latin;
      }
      if (name === 'hex') {
        var hex = '';
        for (var k = 0; k < bytes.length; k += 1) {
          hex += ('0' + bytes[k].toString(16)).slice(-2);
        }
        return hex;
      }
      if (name === 'base64') {
        if (typeof btoa === 'undefined') {
          throw engineError('encodage base64 non supporté', 'unsupported');
        }
        var binary = '';
        for (var m = 0; m < bytes.length; m += 1) {
          binary += String.fromCharCode(bytes[m]);
        }
        return btoa(binary);
      }
      throw engineError('encodage non supporté : ' + encoding, 'unsupported');
    }

    // `class extends Uint8Array` : les instances héritent des slots mémoire
    // d'un `Uint8Array` tout en portant le prototype du shim (donc notre
    // `toString`).
    class ObsidianBuffer extends Uint8Array {
      toString(encoding) {
        return decode(this, encoding);
      }
      toJSON() {
        return {
          type: 'Buffer',
          data: Array.prototype.slice.call(this)
        };
      }
    }
    ObsidianBuffer.from = function (value, encoding) {
      if (typeof value === 'string') {
        return new ObsidianBuffer(encode(value, encoding));
      }
      if (value instanceof ArrayBuffer) {
        return new ObsidianBuffer(new Uint8Array(value));
      }
      if (ArrayBuffer.isView(value)) {
        return new ObsidianBuffer(
          new Uint8Array(value.buffer, value.byteOffset, value.byteLength)
        );
      }
      if (value && value.length !== undefined) {
        return new ObsidianBuffer(value);
      }
      throw engineError('Buffer.from : valeur non supportée', 'invalid_args');
    };
    ObsidianBuffer.alloc = function (size, fill) {
      var buffer = new ObsidianBuffer(size);
      if (fill !== undefined) {
        for (var i = 0; i < buffer.length; i += 1) {
          buffer[i] = fill & 0xff;
        }
      }
      return buffer;
    };
    ObsidianBuffer.concat = function (list, totalLength) {
      var parts = list || [];
      var size = totalLength;
      if (size === undefined) {
        size = 0;
        for (var i = 0; i < parts.length; i += 1) {
          size += parts[i].length;
        }
      }
      var out = new ObsidianBuffer(size);
      var offset = 0;
      for (var j = 0; j < parts.length; j += 1) {
        out.set(parts[j], offset);
        offset += parts[j].length;
      }
      return out;
    };
    ObsidianBuffer.isBuffer = function (value) {
      return value instanceof ObsidianBuffer;
    };
    ObsidianBuffer.byteLength = function (value, encoding) {
      if (typeof value === 'string') {
        return encode(value, encoding).length;
      }
      return value && value.length !== undefined ? value.length : 0;
    };
    return ObsidianBuffer;
  }

  var BufferClass = createBuffer();
  if (BufferClass && !host.Buffer) {
    host.Buffer = BufferClass;
    if (globalScope !== host) {
      globalScope.Buffer = BufferClass;
    }
  }

  /* ------------------------------------------------------------------ */
  /* Modules Node émulés                                                */
  /* ------------------------------------------------------------------ */

  var fsModule = {
    readFileSync: function (file, encoding) {
      if (encoding === undefined || encoding === null) {
        var raw = callSync('fs.readFileSync', [file, 'utf8']);
        return BufferClass ? BufferClass.from(raw, 'utf8') : raw;
      }
      return callSync('fs.readFileSync', [file, String(encoding)]);
    },
    writeFileSync: function (file, data, encoding) {
      return callSync('fs.writeFileSync', [
        file,
        typeof data === 'string' ? data : String(data),
        encoding ? String(encoding) : 'utf8'
      ]);
    },
    existsSync: function (file) {
      try {
        return callSync('fs.existsSync', [file]) === true;
      } catch (err) {
        if (err && err.code === 'not_found') {
          return false;
        }
        // Un refus de confinement est remonté : le renderer ne doit pas
        // croire qu'un fichier hors du vault est simplement absent.
        throw err;
      }
    },
    readdirSync: function (directory, options) {
      return callSync('fs.readdirSync', [directory, options || null]);
    },
    statSync: function (file) {
      var raw = callSync('fs.statSync', [file]);
      return {
        size: raw.size,
        mtime: new Date(raw.mtime * 1000),
        ctime: new Date(raw.mtime * 1000),
        birthtime: new Date(raw.mtime * 1000),
        readonly: raw.readonly,
        isFile: function () {
          return raw.isFile === true;
        },
        isDirectory: function () {
          return raw.isDirectory === true;
        },
        isSymbolicLink: function () {
          return raw.isSymbolicLink === true;
        }
      };
    },
    lstatSync: function (file) {
      return fsModule.statSync(file);
    },
    mkdirSync: function (directory, options) {
      return callSync('fs.mkdirSync', [directory, options || null]);
    },
    unlinkSync: function (file) {
      return callSync('fs.unlinkSync', [file]);
    },
    renameSync: function (from, to) {
      return callSync('fs.renameSync', [from, to]);
    },
    accessSync: function (file) {
      if (!fsModule.existsSync(file)) {
        throw engineError('accès refusé : ' + file, 'not_found');
      }
      return undefined;
    },
    constants: {
      F_OK: 0,
      R_OK: 4,
      W_OK: 2,
      X_OK: 1
    }
  };

  // `fs.promises` : les mêmes opérations, résolues de façon asynchrone.
  fsModule.promises = {
    readFile: function (file, encoding) {
      return Promise.resolve().then(function () {
        return fsModule.readFileSync(file, encoding);
      });
    },
    writeFile: function (file, data, encoding) {
      return Promise.resolve().then(function () {
        return fsModule.writeFileSync(file, data, encoding);
      });
    },
    mkdir: function (directory, options) {
      return Promise.resolve().then(function () {
        return fsModule.mkdirSync(directory, options);
      });
    },
    readdir: function (directory, options) {
      return Promise.resolve().then(function () {
        return fsModule.readdirSync(directory, options);
      });
    },
    stat: function (file) {
      return Promise.resolve().then(function () {
        return fsModule.statSync(file);
      });
    },
    unlink: function (file) {
      return Promise.resolve().then(function () {
        return fsModule.unlinkSync(file);
      });
    },
    rename: function (from, to) {
      return Promise.resolve().then(function () {
        return fsModule.renameSync(from, to);
      });
    }
  };

  /**
   * Normalisation lexicale locale (équivalent de `path.normalize`) : calcul de
   * chaînes, aucun appel au moteur.
   */
  function normalizePath(input) {
    var text = String(input);
    var absolute = text.charAt(0) === '/';
    var parts = text.split('/');
    var out = [];
    for (var i = 0; i < parts.length; i += 1) {
      var part = parts[i];
      if (part === '' || part === '.') {
        continue;
      }
      if (part === '..') {
        if (out.length > 0 && out[out.length - 1] !== '..') {
          out.pop();
        } else if (!absolute) {
          out.push('..');
        }
        continue;
      }
      out.push(part);
    }
    var joined = out.join('/');
    if (absolute) {
      return '/' + joined;
    }
    return joined === '' ? '.' : joined;
  }

  var pathModule = {
    join: function () {
      return callSync(
        'path.join',
        Array.prototype.slice.call(arguments)
      );
    },
    dirname: function (value) {
      return callSync('path.dirname', [value]);
    },
    basename: function (value, extension) {
      return callSync('path.basename', [value, extension || null]);
    },
    extname: function (value) {
      return callSync('path.extname', [value]);
    },
    resolve: function () {
      return callSync(
        'path.resolve',
        Array.prototype.slice.call(arguments)
      );
    },
    normalize: function (value) {
      return normalizePath(value);
    },
    sep: '/',
    delimiter: ':'
  };
  pathModule.posix = pathModule;
  pathModule.win32 = pathModule;

  var cryptoModule = {
    randomUUID: function () {
      return callSync('crypto.randomUUID', []);
    },
    getRandomValues: function (array) {
      var nativeCrypto = host.crypto || globalScope.crypto;
      if (nativeCrypto && typeof nativeCrypto.getRandomValues === 'function') {
        return nativeCrypto.getRandomValues(array);
      }
      for (var i = 0; i < array.length; i += 1) {
        array[i] = Math.floor(Math.random() * 256);
      }
      return array;
    },
    scrypt: function () {
      throw engineError(
        'crypto.scrypt n’est pas implémenté par le moteur obsidian-rs',
        'unsupported'
      );
    }
  };

  var osModule = {
    platform: function () {
      return 'linux';
    },
    arch: function () {
      return 'x64';
    },
    homedir: function () {
      return vaultRoot();
    },
    tmpdir: function () {
      return '/tmp';
    },
    hostname: function () {
      return 'localhost';
    },
    release: function () {
      return '';
    },
    totalmem: function () {
      return 0;
    },
    freemem: function () {
      return 0;
    },
    cpus: function () {
      return [];
    },
    EOL: '\n'
  };

  /* ------------------------------------------------------------------ */
  /* Electron                                                            */
  /* ------------------------------------------------------------------ */

  /** Racine du vault (chemin absolu), demandée au moteur. */
  function vaultRoot() {
    return callSync('ipc.vault.root', []);
  }

  /** Listeners locaux des canaux IPC (`ipcRenderer.on`). */
  var listeners = {};

  function onLocal(channel, listener) {
    if (!listeners[channel]) {
      listeners[channel] = [];
    }
    listeners[channel].push(listener);
    return electronShim.ipcRenderer;
  }

  function emitLocal(channel, args) {
    var registered = listeners[channel] || [];
    for (var i = 0; i < registered.length; i += 1) {
      try {
        registered[i].apply(null, args || []);
      } catch (err) {
        if (host.console && host.console.error) {
          host.console.error('[shim] listener « ' + channel + ' » : ', err);
        }
      }
    }
  }

  /**
   * Canaux `sendSync` du renderer, et leur traduction.
   *
   * Les canaux « vault » sont servis par le moteur (aller-retour synchrone) ;
   * les autres renvoient une valeur par défaut cohérente. Un canal inconnu
   * renvoie `undefined` : le renderer teste lui-même la réponse.
   */
  var SYNC_CHANNELS = {
    vault: function () {
      return { path: vaultRoot(), name: 'vault', open: true };
    },
    'vault-list': function () {
      return {
        vaults: [{ path: vaultRoot(), name: 'vault', open: true, ts: 0 }]
      };
    },
    'vault-open': function () {
      return { path: vaultRoot(), name: 'vault' };
    },
    'get-sandbox-vault-path': function () {
      return vaultRoot();
    },
    'file-url': function (args) {
      return 'file://' + (args && args[0] ? args[0] : vaultRoot());
    },
    'documents-dir': function () {
      return vaultRoot();
    },
    'desktop-dir': function () {
      return vaultRoot();
    },
    version: function () {
      return ENGINE_VERSION;
    },
    frame: function () {
      return { width: 1280, height: 800, x: 0, y: 0, maximized: false };
    },
    'is-quitting': function () {
      return false;
    },
    'is-closing': function () {
      return false;
    },
    sandbox: function () {
      return false;
    },
    'disable-gpu': function () {
      return true;
    },
    'disable-update': function () {
      return true;
    },
    update: function () {
      return false;
    },
    'insider-build': function () {
      return false;
    },
    'check-update': function () {
      return false;
    },
    'latest-public-version': function () {
      return ENGINE_VERSION;
    },
    relaunch: function () {
      return false;
    },
    help: function () {
      return '';
    },
    terms: function () {
      return '';
    },
    policy: function () {
      return '';
    },
    starter: function () {
      return false;
    },
    trash: function () {
      return false;
    },
    cli: function () {
      return '';
    },
    'set-language': function () {
      return '';
    },
    'set-icon': function () {
      return '';
    },
    'get-icon': function () {
      return '';
    },
    'copy-asar': function () {
      return '';
    },
    'adblock-lists': function () {
      return [];
    },
    'adblock-frequency': function () {
      return 0;
    }
  };

  var ipcRenderer = {
    /**
     * Aller-retour **synchrone** vers le moteur (même transport XHR synchrone
     * que les opérations `fs`).
     */
    sendSync: function (channel) {
      var args = Array.prototype.slice.call(arguments, 1);
      var handler = SYNC_CHANNELS[channel];
      if (!handler) {
        return undefined;
      }
      return handler(args);
    },
    /**
     * Message asynchrone : le moteur n'expose pas encore de canal
     * d'événements, le message est donc distribué aux listeners locaux.
     */
    send: function (channel) {
      var args = Array.prototype.slice.call(arguments, 1);
      Promise.resolve().then(function () {
        emitLocal(channel, args);
      });
      return undefined;
    },
    /** Appel asynchrone : renvoie une promesse résolue par la même table. */
    invoke: function (channel) {
      var args = Array.prototype.slice.call(arguments, 1);
      return new Promise(function (resolve) {
        Promise.resolve().then(function () {
          var handler = SYNC_CHANNELS[channel];
          resolve(handler ? handler(args) : null);
        });
      });
    },
    on: function (channel, listener) {
      return onLocal(channel, listener);
    },
    once: function (channel, listener) {
      var wrapper = function () {
        ipcRenderer.removeListener(channel, wrapper);
        listener.apply(null, arguments);
      };
      return onLocal(channel, wrapper);
    },
    removeListener: function (channel, listener) {
      var registered = listeners[channel];
      if (registered) {
        var index = registered.indexOf(listener);
        if (index >= 0) {
          registered.splice(index, 1);
        }
      }
      return electronShim.ipcRenderer;
    },
    removeAllListeners: function (channel) {
      if (channel === undefined) {
        listeners = {};
      } else {
        listeners[channel] = [];
      }
      return electronShim.ipcRenderer;
    }
  };

  var shell = {
    openExternal: function (url) {
      return Promise.resolve('');
    },
    openPath: function (path) {
      return Promise.resolve('');
    },
    showItemInFolder: function (path) {
      return undefined;
    },
    trashItem: function (path) {
      return Promise.resolve('');
    }
  };

  var clipboard = {
    readText: function () {
      return '';
    },
    writeText: function (text) {
      return undefined;
    },
    readImage: function () {
      return null;
    },
    writeImage: function (image) {
      return undefined;
    }
  };

  var dialog = {
    showOpenDialog: function () {
      return Promise.resolve({ canceled: true, filePaths: [] });
    },
    showOpenDialogSync: function () {
      return null;
    },
    showSaveDialog: function () {
      return Promise.resolve({ canceled: true, filePath: undefined });
    },
    showSaveDialogSync: function () {
      return undefined;
    },
    showMessageBox: function () {
      return Promise.resolve({ response: 0 });
    },
    showErrorBox: function () {
      return undefined;
    }
  };

  var app = {
    getPath: function (name) {
      switch (name) {
        case 'home':
        case 'userData':
        case 'documents':
        case 'desktop':
        case 'downloads':
        case 'appData':
          return vaultRoot();
        case 'temp':
          return '/tmp';
        case 'exe':
          return RESOURCES_PATH;
        default:
          return vaultRoot();
      }
    },
    getVersion: function () {
      return ENGINE_VERSION;
    },
    getName: function () {
      return 'Obsidian';
    },
    getLocale: function () {
      return 'fr-FR';
    },
    quit: function () {
      return undefined;
    },
    relaunch: function () {
      return undefined;
    },
    isReady: function () {
      return true;
    }
  };

  var nativeTheme = {
    shouldUseDarkColors: false,
    themeSource: 'system',
    on: function () {
      return nativeTheme;
    },
    removeListener: function () {
      return nativeTheme;
    }
  };

  var webFrame = {
    getZoomLevel: function () {
      return 0;
    },
    setZoomLevel: function () {
      return undefined;
    },
    setFrameZoomLevel: function () {
      return undefined;
    },
    getFrameZoomLevel: function () {
      return 0;
    }
  };

  var electronShim = {
    ipcRenderer: ipcRenderer,
    shell: shell,
    clipboard: clipboard,
    dialog: dialog,
    app: app,
    nativeTheme: nativeTheme,
    webFrame: webFrame,
    safeStorage: {
      isEncryptionAvailable: function () {
        return false;
      },
      encryptString: function () {
        return BufferClass ? BufferClass.alloc(0) : '';
      },
      decryptString: function () {
        return '';
      }
    },
    systemPreferences: {
      isDarkMode: function () {
        return false;
      },
      getMediaAccessStatus: function () {
        return 'denied';
      }
    },
    screen: {
      getAllDisplays: function () {
        return [{ bounds: { x: 0, y: 0, width: 1280, height: 800 } }];
      },
      getPrimaryDisplay: function () {
        return { bounds: { x: 0, y: 0, width: 1280, height: 800 } };
      }
    },
    Menu: {
      buildFromTemplate: function () {
        return { popup: function () {} };
      }
    }
  };

  // `remote` (@electron/remote) : même surface, pour les appels historiques.
  electronShim.remote = {
    app: app,
    clipboard: clipboard,
    dialog: dialog,
    shell: shell,
    nativeTheme: nativeTheme,
    webFrame: webFrame,
    screen: electronShim.screen,
    Menu: electronShim.Menu,
    safeStorage: electronShim.safeStorage,
    systemPreferences: electronShim.systemPreferences,
    getCurrentWebContents: function () {
      return { session: { fromPartition: function () { return {}; } } };
    },
    getCurrentWindow: function () {
      return { id: 1 };
    }
  };

  /* ------------------------------------------------------------------ */
  /* process                                                             */
  /* ------------------------------------------------------------------ */

  var processShim = {
    platform: 'linux',
    arch: 'x64',
    type: 'renderer',
    env: {
      HOME: '/home/obsidian',
      TMPDIR: '/tmp',
      LANG: 'fr_FR.UTF-8',
      ELECTRON_RUN_AS_NODE: '1'
    },
    cwd: function () {
      return vaultRoot();
    },
    versions: {
      node: '20.0.0',
      electron: ENGINE_VERSION,
      chrome: '120.0.0',
      v8: '12.0.0'
    },
    resourcesPath: RESOURCES_PATH,
    argv: ['obsidian'],
    execPath: '/usr/lib/obsidian/obsidian',
    on: function () {
      return processShim;
    },
    once: function () {
      return processShim;
    },
    removeListener: function () {
      return processShim;
    }
  };

  /* ------------------------------------------------------------------ */
  /* require                                                             */
  /* ------------------------------------------------------------------ */

  var utilModule = {
    inherits: function (constructor, superConstructor) {
      constructor.super_ = superConstructor;
      constructor.prototype = Object.create(superConstructor.prototype, {
        constructor: {
          value: constructor,
          enumerable: false,
          writable: true,
          configurable: true
        }
      });
    },
    format: function () {
      return Array.prototype.join.call(arguments, ' ');
    },
    inspect: function (value) {
      return String(value);
    },
    promisify: function (fn) {
      return function () {
        var args = Array.prototype.slice.call(arguments);
        return new Promise(function (resolve, reject) {
          fn.apply(
            null,
            args.concat([
              function (err, value) {
                if (err) {
                  reject(err);
                } else {
                  resolve(value);
                }
              }
            ])
          );
        });
      };
    },
    types: {
      isDate: function (value) {
        return value instanceof Date;
      }
    }
  };

  function EventEmitter() {
    this._events = {};
  }
  EventEmitter.prototype.on = function (name, listener) {
    (this._events[name] = this._events[name] || []).push(listener);
    return this;
  };
  EventEmitter.prototype.once = function (name, listener) {
    var self = this;
    function wrapper() {
      self.removeListener(name, wrapper);
      listener.apply(null, arguments);
    }
    return this.on(name, wrapper);
  };
  EventEmitter.prototype.off = function (name, listener) {
    var registered = this._events[name];
    if (registered) {
      var index = registered.indexOf(listener);
      if (index >= 0) {
        registered.splice(index, 1);
      }
    }
    return this;
  };
  EventEmitter.prototype.removeAllListeners = function (name) {
    if (name === undefined) {
      this._events = {};
    } else {
      this._events[name] = [];
    }
    return this;
  };
  EventEmitter.prototype.emit = function (name) {
    var args = Array.prototype.slice.call(arguments, 1);
    var registered = this._events[name] || [];
    for (var i = 0; i < registered.length; i += 1) {
      registered[i].apply(null, args);
    }
    return registered.length > 0;
  };

  var MODULES = {
    fs: fsModule,
    'original-fs': fsModule,
    path: pathModule,
    crypto: cryptoModule,
    os: osModule,
    electron: electronShim,
    util: utilModule,
    events: EventEmitter,
    buffer: BufferClass
  };

  function requireModule(name) {
    if (Object.prototype.hasOwnProperty.call(MODULES, name)) {
      return MODULES[name];
    }
    throw new Error('module non supporté par le shim obsidian-rs : ' + name);
  }

  /* ------------------------------------------------------------------ */
  /* Installation                                                        */
  /* ------------------------------------------------------------------ */

  function install(target) {
    if (!target) {
      return;
    }
    target.require = requireModule;
    target.process = processShim;
    target.electron = electronShim;
    target.Buffer = target.Buffer || BufferClass;
    target.__OBSIDIAN_ENGINE__ = {
      port: PORT,
      origin: ORIGIN,
      endpoint: ENDPOINT,
      call: callSync,
      modules: Object.keys(MODULES)
    };
  }

  install(host);
  if (globalScope !== host) {
    install(globalScope);
  }
})();
