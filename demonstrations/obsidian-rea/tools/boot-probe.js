/* Harnais de diagnostic v2 (fichier de la box, hors dépôt) :
   - traduit les chemins absolus en chemins relatifs au vault ;
   - extrait la chaîne d'acceptation du bundle au runtime ;
   - complète la surface `electron.remote` absente avec un stub permissif
     et journalise chaque membre manquant (inventaire de la surface). */
(function () {
  'use strict';
  var w = window;
  var log = function () {
    try { console.log('[harnais] ' + Array.prototype.join.call(arguments, ' ')); } catch (e) { /* ignore */ }
  };

  var root = null;
  try { root = w.__OBSIDIAN_ENGINE__.call('ipc.vault.root', []); } catch (e) { log('racine impossible :', e && e.message); }
  log('racine du vault :', root);

  var terms = '';
  try {
    var x = new XMLHttpRequest();
    x.open('GET', 'app.js', false);
    x.send(null);
    var m = /"I understand and agree that I am not allowed to distribute[^"]*"/.exec(x.responseText);
    if (m) { terms = m[0].slice(1, -1); }
  } catch (e) { log('extraction des conditions impossible :', e && e.message); }
  log("chaîne d'acceptation : " + terms.length + " caractères");

  function insideRoot(p) {
    return typeof p === 'string' && !!root && (p === root || p.lastIndexOf(root + '/', 0) === 0);
  }

  var listing = null;
  function rootEntries() {
    if (listing) { return listing; }
    listing = [];
    try {
      var res = w.__OBSIDIAN_ENGINE__.call('ipc.vault.list', []);
      var files = (res && res.files) || [];
      var dirs = {};
      files.forEach(function (f) {
        var path = typeof f === 'string' ? f : (f.path || f.name || '');
        var parts = String(path).split('/');
        if (parts.length > 1) { dirs[parts[0]] = true; }
        else { listing.push({ name: parts[0], isFile: true, isDirectory: false, isSymbolicLink: false }); }
      });
      Object.keys(dirs).forEach(function (d) {
        listing.push({ name: d, isFile: false, isDirectory: true, isSymbolicLink: false });
      });
    } catch (e) { log('listage de la racine impossible :', e && e.message); }
    return listing;
  }

  function patchFs(fsMod) {
    if (!fsMod || fsMod.__harnais) { return fsMod; }
    var patched = {};
    Object.keys(fsMod).forEach(function (k) { patched[k] = fsMod[k]; });
    patched.__harnais = true;
    ['existsSync', 'readFileSync', 'writeFileSync', 'readdirSync', 'statSync',
     'lstatSync', 'mkdirSync', 'unlinkSync', 'renameSync', 'accessSync'].forEach(function (name) {
      var orig = fsMod[name];
      if (typeof orig !== 'function') { return; }
      patched[name] = function (p) {
        var args = Array.prototype.slice.call(arguments);
        if (insideRoot(p)) {
          if (name === 'existsSync' && p === root) { return true; }
          if (name === 'readdirSync' && p === root) {
            var entries = rootEntries();
            var withTypes = args[1] && args[1].withFileTypes;
            return withTypes ? entries : entries.map(function (e) { return e.name; });
          }
          args[0] = p === root ? '.' : p.slice(root.length + 1);
        }
        return orig.apply(fsMod, args);
      };
    });
    return patched;
  }

  var origRequire = w.require;
  w.require = function (id) {
    var mod = origRequire.apply(this, arguments);
    if (id === 'fs' || id === 'original-fs') { return patchFs(mod); }
    if (id === 'os') {
      if (!mod.__harnais) {
        var patchedOs = {};
        Object.keys(mod).forEach(function (k) { patchedOs[k] = mod[k]; });
        patchedOs.__harnais = true;
        if (typeof patchedOs.version !== 'function') {
          patchedOs.version = function () { return '13.0.0'; };
          log('membre manquant : os.version');
          missing['os.version'] = true;
        }
        return patchedOs;
      }
      return mod;
    }
    return mod;
  };

  /* --- surface `electron.remote` absente : stub permissif + inventaire --- */
  var missing = {};
  var DEFAULTS = {
    getVersion: '1.14.4',
    getName: 'Obsidian',
    getAppPath: '/',
    getPath: '/tmp',
    getPreferredSystemLanguages: ['fr-FR'],
    getLocale: 'fr-FR',
    getSystemLocale: 'fr-FR',
    getLoginItemSettings: { openAtLogin: false },
    isPackaged: false,
  };
  function stubFor(name) {
    if (Object.prototype.hasOwnProperty.call(DEFAULTS, name)) { return DEFAULTS[name]; }
    if (/^(getZoom|getVisualZoom|getOpacity|getBackground|getBounds|getSize|getPosition)/.test(name)) { return 1; }
    if (/^is[A-Z]/.test(name)) { return false; }
    if (/^has[A-Z]/.test(name)) { return false; }
    return undefined;
  }
  function makeCallable(path) {
    var fn = function () { return stubFor(path.split('.').pop()); };
    return new Proxy(fn, {
      get: function (obj, prop) {
        if (typeof prop !== 'string') { return obj[prop]; }
        if (prop in obj) { return obj[prop]; }
        if (!(prop in missing)) {
          missing[prop] = path + '.' + prop;
          log('membre manquant :', path + '.' + prop);
        }
        return makeCallable(path + '.' + prop);
      },
      apply: function (obj, thisArg, args) { return stubFor(path.split('.').pop()); },
    });
  }
  function permissive(target, path) {
    return new Proxy(target || {}, {
      get: function (obj, prop) {
        if (typeof prop !== 'string') { return obj[prop]; }
        if (prop in obj) {
          var value = obj[prop];
          if (typeof value === 'function' &&
              (prop === 'getCurrentWebContents' || prop === 'getCurrentWindow')) {
            return function () {
              return permissive(value.apply(obj, arguments), path + '.' + prop + '()');
            };
          }
          return value;
        }
        if (!(prop in missing)) {
          missing[prop] = path + '.' + prop;
          log('membre manquant :', path + '.' + prop);
        }
        return makeCallable(path + '.' + prop);
      },
    });
  }
  var electron = w.electron;
  if (electron && electron.remote) {
    var remote = electron.remote;
    remote.app = permissive(remote.app, 'remote.app');
    electron.app = electron.app ? permissive(electron.app, 'app') : remote.app;
    electron.remote = permissive(remote, 'remote');
  }

  var ipc = w.electron && w.electron.ipcRenderer;
  if (ipc && ipc.sendSync) {
    var origSendSync = ipc.sendSync;
    ipc.sendSync = function (channel) {
      if (channel === 'terms') { return terms; }
      if (channel === 'vault') { return { id: 'harnais', path: root }; }
      if (channel === 'vault-list') {
        return { vaults: [{ id: 'harnais', path: root, name: 'vault', open: true, ts: 0 }] };
      }
      if (channel === 'vault-open') { return { id: 'harnais', path: root }; }
      return origSendSync.apply(this, arguments);
    };
  }
  w.__harnais__ = { root: root, terms: terms, missing: missing };
  log('harnais v2 installé');
})();
