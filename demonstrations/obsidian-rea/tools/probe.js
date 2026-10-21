// Sonde d'état du renderer d'Obsidian : exécutée dans la page via CDP.
// Ne modifie rien, ne renvoie que des observations.
(() => {
  const q = (sel) => document.querySelector(sel);
  const qa = (sel) => Array.from(document.querySelectorAll(sel));
  const txt = (sel) => { const e = q(sel); return e ? (e.textContent || '').trim().slice(0, 120) : null; };
  let bodyHtml = '';
  try { bodyHtml = document.body ? document.body.innerHTML : ''; } catch (e) { bodyHtml = ''; }
  let bodyText = '';
  try { bodyText = document.body ? document.body.innerText : ''; } catch (e) { bodyText = ''; }
  return {
    href: location.href,
    origin: location.origin,
    title: document.title,
    readyState: document.readyState,
    bodyClass: document.body ? document.body.className : null,
    bodyChildren: document.body ? document.body.children.length : -1,
    bodyHtmlLen: bodyHtml.length,
    bodyTextHead: bodyText.slice(0, 300),
    hasRequire: typeof window.require,
    hasElectron: typeof window.electron,
    hasProcess: typeof window.process,
    platform: window.process ? window.process.platform : null,
    electronVersion: window.process ? window.process.versions && window.process.versions.electron : null,
    appContainer: !!q('.app-container'),
    workspace: !!q('.workspace'),
    sidebarLeft: !!q('.workspace-split.mod-left-split'),
    fileTitles: qa('.nav-file-title').map((e) => (e.textContent || '').trim()),
    fileCount: qa('.nav-file-title').length,
    activeFileTitle: txt('.workspace-tab-header-inner') || txt('.nav-file-title.is-active'),
    ribbonIcons: qa('.workspace-ribbon .clickable-icon').length,
    statusBar: txt('.status-bar'),
    modals: qa('.modal').length,
    themeClass: document.body ? document.body.className : null,
    errorOverlay: !!q('.error-overlay'),
    scripts: qa('script[src]').map((s) => s.getAttribute('src')),
  };
})()
