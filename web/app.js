/* ---------------------------------------------------------------------------
   obsy — comportement du front HTML.
   Vanilla JS, sans dépendance ni étape de construction.

   Règle absolue : le front ne touche jamais le disque. Toute lecture,
   écriture, création ou suppression passe par le moteur Rust
   (`src/engine/api.rs`), qui confine les chemins au coffre.

   Transport : l'API REST (`/health`, `/files`, `/file`, `/metadata`,
   `/backlinks`, `/search`, `/graph`) est utilisée en priorité. Le moteur
   n'envoie d'en-têtes CORS que sur `/sync` ; si le navigateur bloque une
   requête REST (origine différente), le front bascule automatiquement sur
   `POST /sync` (`ipc.vault.*`, `fs.*`), qui expose exactement les mêmes
   données. Aucune écriture directe dans le coffre n'est possible dans ce
   front : le seul moyen d'écrire est `PUT /file` ou `ipc.vault.write`.
   --------------------------------------------------------------------------- */

'use strict';

/* ============================ Configuration ============================ */

/** Base de l'API du moteur : `?engine=…`, `window.OBSIDIAN_ENGINE_URL`, puis défaut. */
function resolveEngineBase() {
  const params = new URLSearchParams(window.location.search);
  const fromQuery = params.get('engine') || params.get('api');
  const candidates = [
    fromQuery,
    window.OBSIDIAN_ENGINE_URL,
    (() => { try { return window.localStorage.getItem('obsy:engine'); } catch (_) { return null; } })(),
    'http://127.0.0.1:27123',
  ];
  for (const candidate of candidates) {
    if (typeof candidate === 'string' && candidate.trim() !== '') {
      return candidate.trim().replace(/\/+$/, '');
    }
  }
  return '';
}

const ENGINE_BASE = resolveEngineBase();

/* ============================ État ============================ */

const state = {
  files: [],
  tree: null,
  openTabs: [],
  activePath: null,
  contents: new Map(),      // chemin -> source Markdown
  dirty: false,
  collapsed: new Set(),
  ascending: true,
  activeLeftView: 'files',
  tagMap: new Map(),        // tag -> [chemins]
  links: new Map(),         // cible minuscule -> chemin
  statusTimer: null,
};

const dom = {};

document.addEventListener('DOMContentLoaded', () => {
  cacheDom();
  wireEvents();
  boot();
});

function cacheDom() {
  const ids = {
    vaultName: 'vault-name',
    tree: 'file-tree',
    tabBar: 'tab-bar',
    viewTitle: 'view-title',
    editorInput: 'editor-input',
    editorSource: 'editor-source',
    editorScroll: 'editor-scroll',
    previewContent: 'preview-content',
    previewScroll: 'preview-scroll',
    statusPath: 'status-path',
    statusWords: 'status-words',
    statusLines: 'status-lines',
    statusTheme: 'status-theme',
    statusError: 'status-error',
    backlinksList: 'backlinks-list',
    backlinksCount: 'backlinks-count',
    outlineList: 'outline-list',
    leftBacklinks: 'left-backlinks',
    searchInput: 'search-input',
    searchResults: 'search-results',
    tagList: 'tag-list',
    attachmentList: 'attachment-list',
    sortToggle: 'sort-toggle',
    newNote: 'new-note',
    newFolder: 'new-folder',
    saveNote: 'save-note',
    deleteNote: 'delete-note',
    togglePreview: 'toggle-preview',
    workspace: 'workspace',
    splitter: 'splitter',
    leftDock: 'left-dock',
    navBack: 'nav-back',
    navForward: 'nav-forward',
    toggleLeftDock: 'toggle-left-dock',
    modalVeil: 'modal-veil',
    modalInput: 'modal-input',
    modalList: 'modal-list',
    graphOverlay: 'graph-overlay',
    graphCanvas: 'graph-canvas',
    graphClose: 'graph-close',
    calPrev: 'cal-prev',
    calNext: 'cal-next',
    calTitle: 'cal-title',
    calGrid: 'calendar-grid',
    ribbonQuick: 'ribbon-quick',
    ribbonGraph: 'ribbon-graph',
    ribbonCanvas: 'ribbon-canvas',
    ribbonDaily: 'ribbon-daily',
    ribbonTemplate: 'ribbon-template',
    ribbonCommand: 'ribbon-command',
    ribbonBase: 'ribbon-base',
  };
  for (const [key, id] of Object.entries(ids)) {
    // Les cibles sont repérées par `data-testid` (unique dans la page).
    dom[key] = document.querySelector(`[data-testid="${id}"]`);
  }
  dom.views = Array.from(document.querySelectorAll('.left-dock .view'));
  dom.leftTabs = Array.from(document.querySelectorAll('.dock-tabs-left .dock-tab'));
}

/* ============================ Transport ============================ */

/** Erreur renvoyée par le moteur (statut HTTP ou corps `{"error":…}`). */
class EngineError extends Error {}

/** Erreur de transport : CORS bloquée, réseau coupé, moteur arrêté. */
class TransportError extends Error {}

async function requestJson(url, options) {
  let response;
  try {
    response = await fetch(url, options);
  } catch (cause) {
    const detail = cause && cause.message ? cause.message : 'erreur réseau';
    throw new TransportError(`le moteur n'a pas répondu (${detail})`);
  }
  const text = await response.text();
  let payload = null;
  if (text) {
    try { payload = JSON.parse(text); } catch (_) { payload = null; }
  }
  if (!response.ok) {
    const message = payload && payload.error ? payload.error : `${response.status} ${response.statusText}`;
    throw new EngineError(message);
  }
  if (payload === null) {
    throw new EngineError('réponse illisible');
  }
  return payload;
}

async function restGet(path, params) {
  const query = params ? `?${new URLSearchParams(params).toString()}` : '';
  return requestJson(`${ENGINE_BASE}${path}${query}`, { headers: { Accept: 'application/json' } });
}

async function restPut(path, params, body) {
  const query = params ? `?${new URLSearchParams(params).toString()}` : '';
  return requestJson(`${ENGINE_BASE}${path}${query}`, {
    method: 'PUT',
    headers: { 'Content-Type': 'text/markdown; charset=utf-8' },
    body,
  });
}

/** Appel du pont `/sync` (seule route du moteur porteur d'en-têtes CORS). */
async function syncCall(op, args) {
  const payload = await requestJson(`${ENGINE_BASE}/sync`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ op, args: args || [] }),
  });
  if (!payload || payload.ok !== true) {
    throw new EngineError(payload && payload.error ? payload.error : `opération refusée : ${op}`);
  }
  return payload.value;
}

/**
 * Vrai une fois que le navigateur a refusé une requête REST (le moteur
 * n'envoie d'en-têtes CORS que sur `/sync`) : les appels suivants passent
 * alors directement par le pont, sans nouvelle requête bloquée.
 */
let syncTransportOnly = false;

/**
 * Essaie l'API REST, puis le pont `/sync` si le navigateur a bloqué la
 * requête (origine différente). Le repli n'a lieu que sur erreur de
 * transport : une erreur du moteur (404, 400…) reste une erreur du moteur.
 */
async function viaRest(restCall, syncOp, syncArgs) {
  if (syncTransportOnly && syncOp) {
    return syncCall(syncOp, syncArgs);
  }
  try {
    return await restCall();
  } catch (error) {
    if (error instanceof TransportError && syncOp) {
      syncTransportOnly = true;
      return await syncCall(syncOp, syncArgs);
    }
    throw error;
  }
}

const api = {
  async health() {
    try {
      return await restGet('/health');
    } catch (error) {
      if (error instanceof TransportError) {
        // Le pont `/sync` est la seule route porteuse d'en-têtes CORS :
        // `ipc.vault.root` remplace alors `/health`.
        syncTransportOnly = true;
        const root = await syncCall('ipc.vault.root', []);
        return { ok: true, root: typeof root === 'string' ? root : '', notes: 0 };
      }
      throw error;
    }
  },

  async files() {
    const payload = await viaRest(
      () => restGet('/files'),
      'ipc.vault.list',
      [],
    );
    return (payload && payload.files) || [];
  },

  async read(path) {
    const payload = await viaRest(
      () => restGet('/file', { path }),
      'ipc.vault.read',
      [path],
    );
    return (payload && payload.content) || '';
  },

  async write(path, content) {
    await viaRest(
      () => restPut('/file', { path }, content),
      'ipc.vault.write',
      [path, content],
    );
  },

  async metadata(path) {
    return viaRest(
      () => restGet('/metadata', { path }),
      'ipc.vault.metadata',
      [path],
    );
  },

  async backlinks(path) {
    return viaRest(
      () => restGet('/backlinks', { path }),
      'ipc.vault.backlinks',
      [path],
    );
  },

  async search(query, limit) {
    const payload = await viaRest(
      () => restGet('/search', { q: query, limit: limit || 50 }),
      'ipc.vault.search',
      [query, limit || 50],
    );
    return (payload && payload.results) || [];
  },

  async graph() {
    return viaRest(
      () => restGet('/graph'),
      'ipc.vault.graph',
      [],
    );
  },

  /** Suppression : le moteur n'expose pas de `DELETE /file`, on passe par `/sync`. */
  async remove(path) {
    await syncCall('fs.unlinkSync', [path]);
  },

  /** Création de dossier : également via `/sync` (le moteur confine le chemin). */
  async makeDirectory(path) {
    await syncCall('fs.mkdirSync', [path]);
  },
};

/* ============================ Erreurs discrètes ============================ */

function showError(message) {
  if (!dom.statusError) return;
  dom.statusError.textContent = message;
  dom.statusError.hidden = false;
  if (state.statusTimer) window.clearTimeout(state.statusTimer);
  state.statusTimer = window.setTimeout(() => {
    dom.statusError.hidden = true;
    dom.statusError.textContent = '';
  }, 6000);
}

function clearError() {
  if (!dom.statusError) return;
  dom.statusError.hidden = true;
  dom.statusError.textContent = '';
}

/* ============================ Utilitaires ============================ */

const HTML_ESCAPES = { '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' };

function esc(text) {
  return String(text).replace(/[&<>"']/g, (ch) => HTML_ESCAPES[ch]);
}

function basename(path) {
  const parts = String(path || '').split('/').filter(Boolean);
  return parts.length ? parts[parts.length - 1] : '';
}

function stripExtension(name) {
  return name.replace(/\.[^.]+$/, '');
}

function titleOf(path) {
  return stripExtension(basename(path));
}

function iconUse(name) {
  return `<svg class="icon"><use href="#i-${name}"/></svg>`;
}

/* ============================ Coloration de la source ============================
   Les 18 kinds de `src/vault/editor.rs` (`SourceKind`) sont repris :
   Plain, HeadingMarker, ListMarker, TaskCheckbox, BlockquoteMarker,
   FrontmatterDelimiter, Frontmatter, CodeFence, Emphasis, Strong,
   Strikethrough, InlineCode, Link, WikiLink, Embed, Tag, HtmlComment,
   LineBreak.
   --------------------------------------------------------------------------- */

const INLINE_PATTERN = new RegExp([
  '(?<embed>!\\[\\[[^\\]\\n]*\\]\\])',
  '(?<wikilink>\\[\\[[^\\]\\n]*\\]\\])',
  '(?<code>`[^`\\n]+`)',
  '(?<strong>\\*\\*[^*\\n]+\\*\\*)',
  '(?<strongU>__[^_\\n]+__)',
  '(?<em>\\*[^*\\n]+\\*)',
  '(?<emU>_[^_\\n]+_)',
  '(?<strike>~~[^~\\n]+~~)',
  '(?<link>\\[[^\\]\\n]*\\]\\([^)\\n]*\\))',
  '(?<comment><!--[\\s\\S]*?-->)',
  // Un tag accepte les lettres et chiffres Unicode (`#idée`, `# brouillon/urgent`).
  '(?<tag>(?<![\\w#])#[\\p{L}\\p{N}_\\-/]+)',
].join('|'), 'gu');

/** Cible d'un wikilink, sans son alias. */
function wikilinkTarget(raw) {
  const inner = raw.replace(/^!?\[\[/, '').replace(/\]\]$/, '');
  const bar = inner.indexOf('|');
  return (bar === -1 ? inner : inner.slice(0, bar)).trim();
}

/** Une cible de wikilink pointe-t-elle vers une note connue ? */
function resolvesLink(target) {
  if (!target) return false;
  const key = target.toLowerCase();
  if (state.links.has(key)) return true;
  return state.links.has(stripExtension(key));
}

/** Coloration des éléments en ligne d'une ligne de source. */
function colorInline(text) {
  let out = '';
  let last = 0;
  INLINE_PATTERN.lastIndex = 0;
  let match = INLINE_PATTERN.exec(text);
  while (match !== null) {
    if (match.index > last) out += esc(text.slice(last, match.index));
    const groups = match.groups || {};
    if (groups.embed !== undefined) {
      out += `<span class="tk-embed">${esc(match[0])}</span>`;
    } else if (groups.wikilink !== undefined) {
      const target = wikilinkTarget(match[0]);
      const unresolved = resolvesLink(target) ? '' : ' is-unresolved';
      out += `<span class="tk-wikilink${unresolved}">${esc(match[0])}</span>`;
    } else if (groups.code !== undefined) {
      out += `<span class="tk-inline-code">${esc(match[0])}</span>`;
    } else if (groups.strong !== undefined || groups.strongU !== undefined) {
      out += `<span class="tk-strong">${esc(match[0])}</span>`;
    } else if (groups.em !== undefined) {
      out += `<span class="tk-emphasis">${esc(match[0])}</span>`;
    } else if (groups.emU !== undefined) {
      const before = text[match.index - 1];
      // `_mot_intérieur_` n'est pas de l'emphase (comme dans Obsidian).
      if (before && /[\w]/.test(before)) out += esc(match[0]);
      else out += `<span class="tk-emphasis">${esc(match[0])}</span>`;
    } else if (groups.strike !== undefined) {
      out += `<span class="tk-strikethrough">${esc(match[0])}</span>`;
    } else if (groups.link !== undefined) {
      out += `<span class="tk-link">${esc(match[0])}</span>`;
    } else if (groups.comment !== undefined) {
      out += `<span class="tk-comment">${esc(match[0])}</span>`;
    } else if (groups.tag !== undefined) {
      out += `<span class="tk-tag">${esc(match[0])}</span>`;
    } else {
      out += esc(match[0]);
    }
    last = match.index + match[0].length;
    match = INLINE_PATTERN.exec(text);
  }
  if (last < text.length) out += esc(text.slice(last));
  return out;
}

/** Coloration complète d'une source Markdown (une ligne par fragment). */
function colorSource(source) {
  const lines = String(source).split('\n');
  const rendered = [];
  let inFrontmatter = false;
  let frontmatterClosed = false;
  let inCode = false;
  let fence = '';

  for (let index = 0; index < lines.length; index += 1) {
    const line = lines[index];

    if (!frontmatterClosed && index === 0 && /^---\s*$/.test(line)) {
      inFrontmatter = true;
      rendered.push(`<span class="tk-frontmatter-delim">${esc(line)}</span>`);
      continue;
    }
    if (inFrontmatter) {
      if (/^---\s*$/.test(line)) {
        inFrontmatter = false;
        frontmatterClosed = true;
        rendered.push(`<span class="tk-frontmatter-delim">${esc(line)}</span>`);
      } else {
        rendered.push(`<span class="tk-frontmatter">${esc(line)}</span>`);
      }
      continue;
    }
    if (inCode) {
      if (line.trim().startsWith(fence)) inCode = false;
      rendered.push(`<span class="tk-code-fence">${esc(line)}</span>`);
      continue;
    }
    const fenceMatch = line.match(/^\s*(```|~~~)/);
    if (fenceMatch) {
      inCode = true;
      fence = fenceMatch[1];
      rendered.push(`<span class="tk-code-fence">${esc(line)}</span>`);
      continue;
    }

    let rest = line;
    let prefix = '';
    const take = (pattern, className) => {
      const found = rest.match(pattern);
      if (!found) return;
      prefix += `<span class="${className}">${esc(found[0])}</span>`;
      rest = rest.slice(found[0].length);
    };
    take(/^(#{1,6})(?=\s)/, 'tk-heading-marker');
    take(/^(>\s?)/, 'tk-quote-marker');
    take(/^(\s*)([-*+]|\d+[.)])(?=\s)/, 'tk-list-marker');
    take(/^\[[ xX]\](?=\s)/, 'tk-checkbox');
    rendered.push(prefix + colorInline(rest));
  }
  return rendered.join('\n');
}

/* ============================ Rendu Markdown (aperçu) ============================ */

/** Analyse minimale du frontmatter YAML (clés, valeurs scalaires, listes). */
function parseFrontmatter(source) {
  const lines = String(source).split('\n');
  if (!lines.length || lines[0].trim() !== '---') return null;
  const properties = [];
  let current = null;
  let end = lines.length;
  for (let index = 1; index < lines.length; index += 1) {
    const line = lines[index];
    if (line.trim() === '---') { end = index; break; }
    const pair = line.match(/^([A-Za-z0-9_\-À-ÿ/ ]+):\s*(.*)$/);
    if (pair) {
      current = { key: pair[1].trim(), value: pair[2].trim() };
      properties.push(current);
      continue;
    }
    const item = line.match(/^\s*-\s+(.*)$/);
    if (item && current) {
      current.value = current.value ? `${current.value}, ${item[1].trim()}` : item[1].trim();
    }
  }
  return { properties, bodyStart: end + 1 };
}

/** Rendu des éléments en ligne pour l'aperçu. */
function renderInline(text) {
  let out = '';
  let last = 0;
  INLINE_PATTERN.lastIndex = 0;
  let match = INLINE_PATTERN.exec(text);
  while (match !== null) {
    if (match.index > last) out += esc(text.slice(last, match.index));
    const groups = match.groups || {};
    if (groups.embed !== undefined || groups.wikilink !== undefined) {
      const target = wikilinkTarget(match[0]);
      const unresolved = resolvesLink(target) ? '' : ' is-unresolved';
      const label = match[0]
        .replace(/^!?\[\[/, '')
        .replace(/\]\]$/, '')
        .split('|')
        .pop()
        .trim();
      out += `<span class="md-wikilink${unresolved}" data-wikilink="${esc(target)}">${esc(label)}</span>`;
    } else if (groups.code !== undefined) {
      out += `<code>${esc(match[0].slice(1, -1))}</code>`;
    } else if (groups.strong !== undefined || groups.strongU !== undefined) {
      out += `<strong>${esc(match[0].replace(/^(\*\*|__)|(\*\*|__)$/g, ''))}</strong>`;
    } else if (groups.em !== undefined) {
      out += `<em>${esc(match[0].replace(/^\*|\*$/g, ''))}</em>`;
    } else if (groups.emU !== undefined) {
      const before = text[match.index - 1];
      if (before && /[\w]/.test(before)) out += esc(match[0]);
      else out += `<em>${esc(match[0].replace(/^_|_$/g, ''))}</em>`;
    } else if (groups.strike !== undefined) {
      out += `<del>${esc(match[0].slice(2, -2))}</del>`;
    } else if (groups.link !== undefined) {
      const linkMatch = match[0].match(/^\[([^\]]*)\]\(([^)]*)\)$/);
      const label = linkMatch ? linkMatch[1] : match[0];
      const url = linkMatch ? linkMatch[2] : '';
      out += `<a href="${esc(url)}" target="_blank" rel="noreferrer">${esc(label)}</a>`;
    } else if (groups.comment !== undefined) {
      out += '';
    } else if (groups.tag !== undefined) {
      out += `<span class="md-tag">${esc(match[0])}</span>`;
    } else {
      out += esc(match[0]);
    }
    last = match.index + match[0].length;
    match = INLINE_PATTERN.exec(text);
  }
  if (last < text.length) out += esc(text.slice(last));
  return out;
}

/** Rendu Markdown complet pour l'aperçu. */
function renderMarkdown(source) {
  const lines = String(source).split('\n');
  const front = parseFrontmatter(source);
  const start = front ? front.bodyStart : 0;
  const blocks = [];

  if (front && front.properties.length) {
    const rows = front.properties
      .map((property) => `<div class="property-row"><span class="property-key">${esc(property.key)}</span><span class="property-value">${esc(property.value)}</span></div>`)
      .join('');
    blocks.push(`<div class="properties">${rows}</div>`);
  }

  let index = start;
  let inlineTitleDone = false;
  while (index < lines.length) {
    const line = lines[index];

    if (!line.trim()) { index += 1; continue; }

    const fence = line.match(/^\s*(```|~~~)(.*)$/);
    if (fence) {
      const marker = fence[1];
      const code = [];
      index += 1;
      while (index < lines.length && !lines[index].trim().startsWith(marker)) {
        code.push(lines[index]);
        index += 1;
      }
      index += 1;
      blocks.push(`<pre><code>${esc(code.join('\n'))}</code></pre>`);
      continue;
    }

    const heading = line.match(/^(#{1,6})\s+(.*)$/);
    if (heading) {
      const level = heading[1].length;
      const text = heading[2].trim();
      if (level === 1 && !inlineTitleDone) {
        inlineTitleDone = true;
        blocks.push(`<div class="inline-title" data-heading="${esc(text)}">${renderInline(text)}</div>`);
      } else {
        blocks.push(`<h${level} data-heading="${esc(text)}">${renderInline(text)}</h${level}>`);
      }
      index += 1;
      continue;
    }

    if (/^\s*(\*\s*){3,}$/.test(line) || /^\s*(-\s*){3,}$/.test(line)) {
      blocks.push('<hr>');
      index += 1;
      continue;
    }

    if (/^\s*\|/.test(line) && index + 1 < lines.length && /^\s*\|?[\s:-]*\|/.test(lines[index + 1])) {
      const table = [];
      while (index < lines.length && /^\s*\|/.test(lines[index])) {
        table.push(lines[index]);
        index += 1;
      }
      blocks.push(renderTable(table));
      continue;
    }

    if (/^\s*>/.test(line)) {
      const quoted = [];
      while (index < lines.length && /^\s*>/.test(lines[index])) {
        quoted.push(lines[index].replace(/^\s*>\s?/, ''));
        index += 1;
      }
      const callout = quoted[0] ? quoted[0].match(/^\[!([A-Za-z]+)\](?:\s+(.*))?$/) : null;
      if (callout) {
        const kind = callout[1];
        const title = callout[2] || kind;
        const body = renderMarkdown(quoted.slice(1).join('\n'));
        blocks.push(`<div class="callout"><div class="callout-title">${esc(title)}</div>${body}</div>`);
      } else {
        blocks.push(`<blockquote>${renderMarkdown(quoted.join('\n'))}</blockquote>`);
      }
      continue;
    }

    if (/^\s*([-*+]|\d+[.)])\s+/.test(line)) {
      const ordered = /^\s*\d+[.)]\s+/.test(line);
      const items = [];
      while (index < lines.length) {
        const item = lines[index].match(/^\s*([-*+]|\d+[.)])\s+(.*)$/);
        if (!item) break;
        const task = item[2].match(/^\[([ xX])\]\s*(.*)$/);
        if (task) {
          const done = task[1].toLowerCase() === 'x';
          items.push(`<li class="task${done ? ' is-done' : ''}"><span class="task-checkbox">${done ? '✓' : ''}</span><span class="task-label">${renderInline(task[2])}</span></li>`);
        } else {
          items.push(`<li>${renderInline(item[2])}</li>`);
        }
        index += 1;
      }
      const tag = ordered ? 'ol' : 'ul';
      blocks.push(`<${tag}>${items.join('')}</${tag}>`);
      continue;
    }

    const paragraph = [];
    while (index < lines.length && lines[index].trim() && !/^\s*(#{1,6}\s|>|```|~~~|([-*+]|\d+[.)])\s)/.test(lines[index])) {
      paragraph.push(lines[index]);
      index += 1;
    }
    if (paragraph.length) {
      blocks.push(`<p>${renderInline(paragraph.join('\n'))}</p>`);
    } else {
      index += 1;
    }
  }
  return blocks.join('\n');
}

/** Tableau Markdown. */
function renderTable(rows) {
  const cells = (row) => row.trim().replace(/^\|/, '').replace(/\|$/, '').split('|').map((cell) => cell.trim());
  const head = cells(rows[0]);
  const body = rows.slice(2).map(cells);
  const headHtml = head.map((cell) => `<th>${renderInline(cell)}</th>`).join('');
  const bodyHtml = body.map((row) => `<tr>${row.map((cell) => `<td>${renderInline(cell)}</td>`).join('')}</tr>`).join('');
  return `<table><thead><tr>${headHtml}</tr></thead><tbody>${bodyHtml}</tbody></table>`;
}

/* ============================ Arbre de fichiers ============================ */

/** Construit l'arborescence à partir de la liste plate renvoyée par `/files`. */
function buildTree(files) {
  const root = { name: '', path: '', dirs: new Map(), files: [] };
  for (const file of files) {
    const parts = String(file.path).split('/').filter(Boolean);
    if (!parts.length) continue;
    let node = root;
    for (let index = 0; index < parts.length - 1; index += 1) {
      const name = parts[index];
      if (!node.dirs.has(name)) {
        node.dirs.set(name, { name, path: parts.slice(0, index + 1).join('/'), dirs: new Map(), files: [] });
      }
      node = node.dirs.get(name);
    }
    node.files.push({ name: parts[parts.length - 1], path: file.path });
  }
  return root;
}

/** Index des cibles de wikilink (nom de note sans extension, chemin complet). */
function buildLinkIndex(files) {
  state.links = new Map();
  for (const file of files) {
    const path = file.path;
    state.links.set(path.toLowerCase(), path);
    state.links.set(stripExtension(path).toLowerCase(), path);
    state.links.set(titleOf(path).toLowerCase(), path);
  }
}

function renderTree() {
  const container = dom.tree;
  if (!container) return;
  container.textContent = '';
  if (!state.tree) return;
  const fragment = document.createDocumentFragment();
  renderTreeLevel(state.tree, fragment);
  container.appendChild(fragment);
}

function renderTreeLevel(node, container) {
  const dirs = Array.from(node.dirs.values()).sort((left, right) => compareNames(left.name, right.name));
  const files = node.files.slice().sort((left, right) => compareNames(left.name, right.name));
  for (const dir of dirs) {
    container.appendChild(renderTreeDirectory(dir));
  }
  for (const file of files) {
    container.appendChild(renderTreeFile(file));
  }
}

function compareNames(left, right) {
  const result = left.localeCompare(right, 'fr', { numeric: true, sensitivity: 'base' });
  return state.ascending ? result : -result;
}

function renderTreeDirectory(dir) {
  const item = document.createElement('li');
  const row = document.createElement('div');
  row.className = 'tree-item';
  row.dataset.testid = 'tree-item';
  row.dataset.path = dir.path;
  row.dataset.kind = 'folder';

  const collapsed = state.collapsed.has(dir.path);
  const disclosure = document.createElement('span');
  disclosure.className = `tree-item-disclosure${collapsed ? ' is-collapsed' : ''}`;
  disclosure.innerHTML = iconUse('chevron');
  disclosure.addEventListener('click', (event) => {
    event.stopPropagation();
    toggleFolder(dir.path);
  });

  row.appendChild(disclosure);
  row.insertAdjacentHTML('beforeend', iconUse('files'));
  const label = document.createElement('span');
  label.className = 'tree-item-label';
  label.textContent = dir.name;
  row.appendChild(label);
  row.addEventListener('click', () => toggleFolder(dir.path));

  const list = document.createElement('ul');
  if (!collapsed) renderTreeLevel(dir, list);
  item.appendChild(row);
  item.appendChild(list);
  return item;
}

function renderTreeFile(file) {
  const item = document.createElement('li');
  const row = document.createElement('div');
  row.className = 'tree-item';
  row.dataset.testid = 'tree-item';
  row.dataset.path = file.path;
  row.dataset.kind = 'file';
  if (file.path === state.activePath) row.classList.add('is-active');
  row.insertAdjacentHTML('beforeend', iconUse(file.name.endsWith('.md') ? 'file' : 'paperclip'));
  const label = document.createElement('span');
  label.className = 'tree-item-label';
  label.textContent = file.name;
  row.appendChild(label);
  row.addEventListener('click', () => openNote(file.path));
  item.appendChild(row);
  return item;
}

function toggleFolder(path) {
  if (state.collapsed.has(path)) state.collapsed.delete(path);
  else state.collapsed.add(path);
  renderTree();
}

/* ============================ Onglets de notes ============================ */

function renderTabs() {
  const bar = dom.tabBar;
  if (!bar) return;
  bar.textContent = '';
  state.openTabs.forEach((path, index) => {
    const tab = document.createElement('div');
    tab.className = `note-tab${index === state.openTabs.length - 1 && path === state.activePath ? ' is-active' : ''}`;
    tab.dataset.testid = 'note-tab';
    tab.dataset.path = path;
    tab.dataset.dirty = state.dirty && path === state.activePath ? 'true' : 'false';

    const inner = document.createElement('div');
    inner.className = 'note-tab-inner';

    const title = document.createElement('span');
    title.className = 'note-tab-title';
    title.textContent = titleOf(path);
    title.addEventListener('click', () => openNote(path));

    const close = document.createElement('button');
    close.className = 'note-tab-close';
    close.dataset.testid = 'tab-close';
    close.title = 'Fermer';
    close.innerHTML = iconUse('close');
    close.addEventListener('click', (event) => {
      event.stopPropagation();
      closeTab(path);
    });

    inner.appendChild(title);
    inner.appendChild(close);
    tab.appendChild(inner);
    tab.addEventListener('click', () => openNote(path));
    bar.appendChild(tab);
  });
}

function closeTab(path) {
  const index = state.openTabs.indexOf(path);
  if (index === -1) return;
  state.openTabs.splice(index, 1);
  state.contents.delete(path);
  if (state.activePath === path) {
    const next = state.openTabs[state.openTabs.length - 1];
    if (next) openNote(next);
    else {
      state.activePath = null;
      dom.editorInput.value = '';
      dom.editorSource.innerHTML = '';
      dom.previewContent.innerHTML = '';
      dom.viewTitle.textContent = 'Aucune note';
      updateStatus();
      renderTabs();
      renderTree();
    }
  } else {
    renderTabs();
  }
}

/* ============================ Éditeur ============================ */

function setEditorContent(text) {
  dom.editorInput.value = text;
  dom.editorSource.innerHTML = colorSource(text);
  growEditor();
}

function growEditor() {
  dom.editorInput.style.height = 'auto';
  dom.editorInput.style.height = `${dom.editorInput.scrollHeight}px`;
}

function updateHighlight() {
  dom.editorSource.innerHTML = colorSource(dom.editorInput.value);
  growEditor();
}

/* ============================ Aperçu et plan ============================ */

/** Rafraîchit l'aperçu et le plan de la note depuis la source courante. */
function refreshPreview() {
  const content = dom.editorInput.value;
  dom.previewContent.innerHTML = renderMarkdown(content);
  renderOutline(content);
}

let previewTimer = null;

function schedulePreviewRefresh() {
  if (previewTimer) window.clearTimeout(previewTimer);
  previewTimer = window.setTimeout(refreshPreview, 180);
}

/* ============================ Barre d'état ============================ */

function updateStatus() {
  const content = state.activePath ? (state.contents.get(state.activePath) || '') : '';
  const words = content.trim() ? content.trim().split(/\s+/).length : 0;
  const lines = content ? content.split('\n').length : 0;
  dom.statusWords.textContent = `${words} mot${words > 1 ? 's' : ''}`;
  dom.statusLines.textContent = `${lines} ligne${lines > 1 ? 's' : ''}`;
  dom.statusPath.textContent = state.activePath || '—';
  dom.statusTheme.textContent = state.theme === 'light' ? 'Clair' : 'Sombre';
}

/* ============================ Ouverture d'une note ============================ */

async function openNote(path, opts) {
  if (!path) return;
  try {
    if (!state.contents.has(path)) {
      const content = await api.read(path);
      state.contents.set(path, content);
    }
    state.activePath = path;
    if (!state.openTabs.includes(path)) state.openTabs.push(path);
    state.dirty = false;
    if (!(opts && opts.fromHistory)) pushHistory(path);
    touchRecent(path);

    const content = state.contents.get(path) || '';
    setEditorContent(content);
    refreshPreview();
    dom.viewTitle.textContent = titleOf(path);
    window.location.hash = `#${encodeURIComponent(path)}`;

    renderTabs();
    renderTree();
    updateStatus();
    clearError();

    await refreshSidePanels(path, content);
  } catch (error) {
    showError(`ouverture impossible : ${error.message}`);
  }
}

/** Plan de la note : titres de la source, cliquables dans l'aperçu. */
function renderOutline(content) {
  const outline = dom.outlineList;
  if (!outline) return;
  outline.textContent = '';
  for (const heading of extractHeadings(content)) {
    const item = document.createElement('li');
    item.className = `result-item outline-level-${heading.level}`;
    item.dataset.testid = 'outline-item';
    item.textContent = heading.text;
    item.addEventListener('click', () => {
      const target = Array.from(dom.previewContent.querySelectorAll('[data-heading]'))
        .find((node) => node.dataset.heading === heading.text);
      if (target && dom.previewScroll) {
        dom.previewScroll.scrollTop = target.offsetTop - dom.previewScroll.offsetTop - 16;
      }
    });
    outline.appendChild(item);
  }
}

/** Rétroliens et plan de la note. */
async function refreshSidePanels(path, content) {
  const backlinks = dom.backlinksList;
  backlinks.textContent = '';
  renderOutline(content);

  try {
    const payload = await api.backlinks(path);
    const entries = (payload && payload.backlinks) || [];
    dom.backlinksCount.textContent = String(entries.length);
    for (const entry of entries) {
      const item = document.createElement('li');
      item.className = 'result-item';
      item.dataset.testid = 'backlink-item';
      item.dataset.path = entry.from;
      item.innerHTML = `${iconUse('file')}<span class="result-item-title">${esc(titleOf(entry.from))}</span><span class="result-item-meta">${entry.count}</span>`;
      item.addEventListener('click', () => openNote(entry.from));
      backlinks.appendChild(item);
    }
    if (state.activeLeftView === 'backlinks') renderLeftBacklinks(entries);
  } catch (error) {
    showError(`rétroliens indisponibles : ${error.message}`);
  }
}

/** Titres d'une note (hors blocs de code). */
function extractHeadings(source) {
  const headings = [];
  let inCode = false;
  let fence = '';
  for (const line of String(source).split('\n')) {
    const fenceMatch = line.match(/^\s*(```|~~~)/);
    if (fenceMatch) {
      if (!inCode) { inCode = true; fence = fenceMatch[1]; }
      else if (line.trim().startsWith(fence)) { inCode = false; }
      continue;
    }
    if (inCode) continue;
    const match = line.match(/^(#{1,6})\s+(.*)$/);
    if (match) headings.push({ level: match[1].length, text: match[2].trim() });
  }
  return headings;
}

/* ============================ Ouverture d'une note ============================ */

function selectLeftView(view) {
  state.activeLeftView = view;
  for (const section of dom.views) {
    section.classList.toggle('is-active', section.dataset.view === view);
  }
  for (const tab of dom.leftTabs) {
    tab.classList.toggle('is-active', tab.dataset.view === view);
  }
  if (view === 'tags') loadTags();
  if (view === 'attachments') renderAttachments();
  if (view === 'backlinks') renderLeftBacklinksFromState();
  if (view === 'calendar') renderCalendar();
}

function renderLeftBacklinks(entries) {
  const list = dom.leftBacklinks;
  if (!list) return;
  list.textContent = '';
  for (const entry of entries || []) {
    const item = document.createElement('li');
    item.className = 'result-item';
    item.dataset.testid = 'left-backlink-item';
    item.innerHTML = `${iconUse('file')}<span class="result-item-title">${esc(titleOf(entry.from))}</span>`;
    item.addEventListener('click', () => openNote(entry.from));
    list.appendChild(item);
  }
}

function renderLeftBacklinksFromState() {
  const list = dom.leftBacklinks;
  if (!list) return;
  list.textContent = '';
  if (!state.activePath) return;
  api.backlinks(state.activePath)
    .then((payload) => renderLeftBacklinks((payload && payload.backlinks) || []))
    .catch((error) => showError(`rétroliens indisponibles : ${error.message}`));
}

async function loadTags() {
  const list = dom.tagList;
  if (!list) return;
  list.textContent = '';
  const tags = new Map();
  try {
    for (const file of state.files) {
      if (!file.path.endsWith('.md')) continue;
      try {
        const metadata = await api.metadata(file.path);
        for (const tag of metadata.tags || []) {
          if (!tags.has(tag)) tags.set(tag, []);
          tags.get(tag).push(file.path);
        }
      } catch (_) {
        /* une note illisible n'interrompt pas la liste */
      }
    }
    state.tagMap = tags;
    const names = Array.from(tags.keys()).sort((left, right) => left.localeCompare(right, 'fr'));
    for (const tag of names) {
      const item = document.createElement('li');
      item.className = 'result-item';
      item.dataset.testid = 'tag-item';
      item.innerHTML = `${iconUse('tag')}<span class="result-item-title">#${esc(tag)}</span><span class="result-item-meta">${tags.get(tag).length}</span>`;
      item.addEventListener('click', () => {
        selectLeftView('files');
        dom.searchInput.value = `#${tag}`;
        runSearch(`#${tag}`);
        selectLeftView('search');
      });
      list.appendChild(item);
    }
    if (!names.length) {
      const empty = document.createElement('li');
      empty.className = 'empty-state';
      empty.textContent = 'Aucun tag dans ce coffre.';
      list.appendChild(empty);
    }
  } catch (error) {
    showError(`tags indisponibles : ${error.message}`);
  }
}

function renderAttachments() {
  const list = dom.attachmentList;
  if (!list) return;
  list.textContent = '';
  const attachments = state.files.filter((file) => !file.path.endsWith('.md'));
  for (const file of attachments) {
    const item = document.createElement('li');
    item.className = 'result-item';
    item.dataset.testid = 'attachment-item';
    item.innerHTML = `${iconUse('paperclip')}<span class="result-item-title">${esc(file.name)}</span>`;
    item.addEventListener('click', () => openNote(file.path));
    list.appendChild(item);
  }
  if (!attachments.length) {
    const empty = document.createElement('li');
    empty.className = 'empty-state';
    empty.textContent = 'Aucune pièce jointe.';
    list.appendChild(empty);
  }
}

/* ============================ Recherche ============================ */

let searchTimer = null;

function runSearch(query) {
  if (!query) {
    dom.searchResults.textContent = '';
    return;
  }
  api.search(query, 50)
    .then((results) => {
      dom.searchResults.textContent = '';
      if (!results.length) {
        const empty = document.createElement('li');
        empty.className = 'empty-state';
        empty.textContent = 'Aucun résultat.';
        dom.searchResults.appendChild(empty);
        return;
      }
      for (const result of results) {
        const item = document.createElement('li');
        item.className = 'result-item';
        item.dataset.testid = 'search-result';
        item.dataset.path = result.path;
        item.innerHTML = `${iconUse('file')}<span class="result-item-title">${esc(titleOf(result.path))}</span><span class="result-item-meta">l. ${result.line}</span><span class="result-item-text">${esc(result.text || '')}</span>`;
        item.addEventListener('click', () => openNote(result.path));
        dom.searchResults.appendChild(item);
      }
    })
    .catch((error) => showError(`recherche impossible : ${error.message}`));
}

/* ============================ Écriture ============================ */

async function saveNote() {
  if (!state.activePath) return;
  const content = dom.editorInput.value;
  try {
    await api.write(state.activePath, content);
    state.contents.set(state.activePath, content);
    state.dirty = false;
    const activeTab = dom.tabBar
      ? dom.tabBar.querySelector(`[data-path="${CSS.escape(state.activePath)}"]`)
      : null;
    if (activeTab) activeTab.dataset.dirty = 'false';
    updateStatus();
    clearError();
  } catch (error) {
    showError(`enregistrement impossible : ${error.message}`);
  }
}

async function createNote() {
  const name = window.prompt('Nom de la nouvelle note', 'Nouvelle note');
  if (!name) return;
  const fileName = name.endsWith('.md') ? name : `${name}.md`;
  const path = fileName;
  try {
    await api.write(path, `# ${titleOf(path)}\n`);
    await refreshFiles();
    await openNote(path);
  } catch (error) {
    showError(`création impossible : ${error.message}`);
  }
}

async function createFolder() {
  const name = window.prompt('Nom du nouveau dossier', 'Nouveau dossier');
  if (!name) return;
  const path = name.replace(/^\/+|\/+$/g, '');
  if (!path) return;
  try {
    await api.makeDirectory(path);
    await refreshFiles();
  } catch (error) {
    showError(`création du dossier impossible : ${error.message}`);
  }
}

async function deleteNote() {
  const path = state.activePath;
  if (!path) return;
  const confirmed = window.confirm(`Supprimer la note « ${titleOf(path)} » ?`);
  if (!confirmed) return;
  try {
    await api.remove(path);
    state.contents.delete(path);
    state.openTabs = state.openTabs.filter((tab) => tab !== path);
    await refreshFiles();
    const next = state.openTabs[state.openTabs.length - 1];
    if (next) await openNote(next);
    else {
      state.activePath = null;
      setEditorContent('');
      dom.previewContent.innerHTML = '';
      dom.viewTitle.textContent = 'Aucune note';
      renderTabs();
      updateStatus();
    }
    clearError();
  } catch (error) {
    showError(`suppression impossible : ${error.message}`);
  }
}

/* ============================ Chargement du coffre ============================ */

async function refreshFiles() {
  try {
    state.files = await api.files();
    state.tree = buildTree(state.files);
    buildLinkIndex(state.files);
    renderTree();
    renderCalendar();
  } catch (error) {
    showError(`coffre illisible : ${error.message}`);
  }
}

async function boot() {
  try {
    const health = await api.health();
    const root = health && health.root ? health.root : '';
    dom.vaultName.textContent = basename(root) || 'obsy';
  } catch (error) {
    dom.vaultName.textContent = 'obsy';
    showError(`moteur injoignable : ${error.message}`);
  }
  await refreshFiles();

  const fromHash = decodeURIComponent(window.location.hash.replace(/^#/, ''));
  const notes = state.files.filter((file) => file.path.endsWith('.md'));
  const first = notes[0] || state.files[0];
  const target = state.files.some((file) => file.path === fromHash)
    ? fromHash
    : (first ? first.path : null);
  if (target) await openNote(target);
  else {
    dom.tree.innerHTML = '<li class="empty-state">Coffre vide.</li>';
  }
}

/* ============================ Événements ============================ */

function wireEvents() {
  for (const tab of dom.leftTabs) {
    tab.addEventListener('click', () => selectLeftView(tab.dataset.view));
  }
  for (const tab of document.querySelectorAll('.dock-tabs-right .dock-tab')) {
    tab.addEventListener('click', () => {
      const tabs = Array.from(document.querySelectorAll('.dock-tabs-right .dock-tab'));
      const panels = Array.from(document.querySelectorAll('.right-dock .side-panel'));
      const index = tabs.indexOf(tab);
      tabs.forEach((other, otherIndex) => other.classList.toggle('is-active', otherIndex === index));
      const panel = panels[index];
      if (panel) panel.scrollIntoView({ block: 'start' });
    });
  }

  dom.sortToggle.addEventListener('click', () => {
    state.ascending = !state.ascending;
    dom.sortToggle.textContent = state.ascending ? 'Tri A→Z' : 'Tri Z→A';
    renderTree();
  });
  dom.newNote.addEventListener('click', createNote);
  dom.newFolder.addEventListener('click', createFolder);
  dom.saveNote.addEventListener('click', saveNote);
  dom.deleteNote.addEventListener('click', deleteNote);
  dom.togglePreview.addEventListener('click', togglePreviewView);

  dom.searchInput.addEventListener('input', () => {
    if (searchTimer) window.clearTimeout(searchTimer);
    const query = dom.searchInput.value.trim();
    searchTimer = window.setTimeout(() => runSearch(query), 200);
  });

  dom.editorInput.addEventListener('input', () => {
    state.dirty = true;
    const activeTab = dom.tabBar
      ? dom.tabBar.querySelector(`[data-path="${CSS.escape(state.activePath || '')}"]`)
      : null;
    if (activeTab) activeTab.dataset.dirty = 'true';
    updateHighlight();
    updateStatus();
    // L'aperçu et le plan suivent la frappe : sans cela, la source et
    // l'aperçu divergent.
    schedulePreviewRefresh();
  });

  dom.editorInput.addEventListener('keydown', (event) => {
    if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 's') {
      event.preventDefault();
      saveNote();
    }
  });

  dom.previewContent.addEventListener('click', (event) => {
    const target = event.target.closest('[data-wikilink]');
    if (!target) return;
    const wanted = target.dataset.wikilink.toLowerCase();
    const path = state.links.get(wanted) || state.links.get(stripExtension(wanted));
    if (path) openNote(path);
    else showError(`lien non résolu : ${target.dataset.wikilink}`);
  });

  window.addEventListener('hashchange', () => {
    const path = decodeURIComponent(window.location.hash.replace(/^#/, ''));
    if (path && path !== state.activePath && state.files.some((file) => file.path === path)) {
      openNote(path);
    }
  });

  wireSplitter();
}

/** Séparateur redimensionnable (3 px) entre l'éditeur et l'aperçu. */
function wireSplitter() {
  let dragging = false;
  dom.splitter.addEventListener('pointerdown', (event) => {
    dragging = true;
    dom.splitter.setPointerCapture(event.pointerId);
    event.preventDefault();
  });
  dom.splitter.addEventListener('pointermove', (event) => {
    if (!dragging) return;
    const workspace = dom.workspace.getBoundingClientRect();
    const width = Math.min(Math.max(event.clientX - workspace.left, 240), workspace.width - 240);
    const editorPane = dom.workspace.querySelector('.editor-pane');
    if (editorPane) {
      editorPane.style.flex = `0 0 ${width}px`;
      editorPane.style.maxWidth = `${width}px`;
    }
  });
  const stop = (event) => {
    if (!dragging) return;
    dragging = false;
    try { dom.splitter.releasePointerCapture(event.pointerId); } catch (_) { /* déjà relâché */ }
  };
  dom.splitter.addEventListener('pointerup', stop);
  dom.splitter.addEventListener('pointercancel', stop);
}

/* ============================ Thème clair / sombre ============================ */

state.theme = (() => {
  try { return window.localStorage.getItem('obsy:theme') === 'light' ? 'light' : 'dark'; }
  catch (_) { return 'dark'; }
})();

function applyTheme(theme) {
  state.theme = theme === 'light' ? 'light' : 'dark';
  document.body.classList.toggle('theme-light', state.theme === 'light');
  document.body.classList.toggle('theme-dark', state.theme === 'dark');
  if (dom.statusTheme) dom.statusTheme.textContent = state.theme === 'light' ? 'Clair' : 'Sombre';
  try { window.localStorage.setItem('obsy:theme', state.theme); } catch (_) { /* stockage refusé */ }
}

function toggleTheme() {
  applyTheme(state.theme === 'light' ? 'dark' : 'light');
}

/* ============================ Historique de navigation ============================ */

state.history = [];
state.historyIndex = -1;

function pushHistory(path) {
  if (state.history[state.historyIndex] === path) return;
  state.history = state.history.slice(0, state.historyIndex + 1);
  state.history.push(path);
  state.historyIndex = state.history.length - 1;
  updateNavButtons();
}

function goHistory(delta) {
  const target = state.historyIndex + delta;
  if (target < 0 || target >= state.history.length) return;
  state.historyIndex = target;
  updateNavButtons();
  openNote(state.history[target], { fromHistory: true });
}

function updateNavButtons() {
  if (dom.navBack) dom.navBack.disabled = state.historyIndex <= 0;
  if (dom.navForward) dom.navForward.disabled = state.historyIndex >= state.history.length - 1;
}

/* ============================ Notes récentes ============================ */

state.recent = (() => {
  try {
    const parsed = JSON.parse(window.localStorage.getItem('obsy:recent') || '[]');
    return Array.isArray(parsed) ? parsed.filter((p) => typeof p === 'string') : [];
  } catch (_) { return []; }
})();

function touchRecent(path) {
  state.recent = [path].concat(state.recent.filter((p) => p !== path)).slice(0, 20);
  try { window.localStorage.setItem('obsy:recent', JSON.stringify(state.recent)); } catch (_) { /* stockage refusé */ }
}

/* ============================ Lecture / édition ============================ */

function togglePreviewView() {
  if (!dom.workspace) return;
  const previewPane = dom.workspace.querySelector('.preview-pane');
  if (!previewPane) return;
  const hidden = previewPane.style.display === 'none';
  previewPane.style.display = hidden ? 'flex' : 'none';
  if (dom.splitter) dom.splitter.style.display = hidden ? 'block' : 'none';
}

/* ============================ Recherche globale (focus) ============================ */

function focusGlobalSearch() {
  selectLeftView('search');
  if (dom.searchInput) {
    dom.searchInput.focus();
    dom.searchInput.select();
  }
}

/* ============================ Invite : sélecteur rapide / palette ============================ */

const promptState = { mode: null, items: [], index: 0 };

/** Score de filtrage approximatif (sous-séquence, bonus débuts de mot et suites). */
function fuzzyScore(query, text) {
  if (!query) return 1;
  const q = query.toLowerCase();
  const t = text.toLowerCase();
  let qi = 0;
  let score = 0;
  let streak = 0;
  let last = -2;
  for (let ti = 0; ti < t.length && qi < q.length; ti += 1) {
    if (t[ti] === q[qi]) {
      streak = ti === last + 1 ? streak + 1 : 1;
      const wordStart = ti === 0 || /[\s\-_/]/.test(t[ti - 1]);
      score += 1 + streak * 2 + (wordStart ? 4 : 0);
      last = ti;
      qi += 1;
    }
  }
  return qi === q.length ? score : -1;
}

const COMMANDS = [
  { name: 'Nouvelle note', kbd: 'Ctrl+N', icon: 'file-plus', run: () => createNote() },
  { name: 'Nouveau dossier', icon: 'folder-plus', run: () => createFolder() },
  { name: 'Note quotidienne', icon: 'daily', run: () => openDailyNote() },
  { name: 'Sélecteur rapide…', kbd: 'Ctrl+K', icon: 'search', run: () => openPrompt('switcher') },
  { name: 'Vue graphique', kbd: 'Ctrl+G', icon: 'graph', run: () => openGraph() },
  { name: 'Basculer lecture / édition', icon: 'edit', run: () => togglePreviewView() },
  { name: 'Basculer thème clair / sombre', icon: 'panel', run: () => toggleTheme() },
  { name: 'Enregistrer la note', kbd: 'Ctrl+S', icon: 'check', run: () => saveNote() },
  { name: 'Supprimer la note', icon: 'trash', run: () => deleteNote() },
  { name: 'Recherche globale', kbd: 'Ctrl+Shift+F', icon: 'search', run: () => focusGlobalSearch() },
];

/** Entrées du sélecteur rapide : notes récentes d'abord, puis ordre alphabétique. */
function switcherItems() {
  const notes = state.files.filter((file) => file.path.endsWith('.md'));
  const rank = new Map(state.recent.map((path, index) => [path, index]));
  return notes
    .map((file) => ({
      icon: 'file',
      title: titleOf(file.path),
      meta: file.path,
      kbd: '',
      run: () => openNote(file.path),
      sort: rank.has(file.path) ? rank.get(file.path) - 1000 : 0,
    }))
    .sort((left, right) => left.sort - right.sort || left.title.localeCompare(right.title, 'fr'));
}

function commandItems() {
  return COMMANDS.map((command) => ({
    icon: command.icon,
    title: command.name,
    meta: '',
    kbd: command.kbd || '',
    run: command.run,
  }));
}

function openPrompt(mode) {
  if (!dom.modalVeil || !dom.modalInput) return;
  promptState.mode = mode;
  promptState.index = 0;
  dom.modalVeil.hidden = false;
  dom.modalInput.value = '';
  dom.modalInput.placeholder = mode === 'switcher' ? 'Aller à un fichier…' : 'Exécuter une commande…';
  renderPrompt('');
  dom.modalInput.focus();
}

function closePrompt() {
  promptState.mode = null;
  if (dom.modalVeil) dom.modalVeil.hidden = true;
}

function promptResults(query) {
  const base = promptState.mode === 'switcher' ? switcherItems() : commandItems();
  if (!query) return base.slice(0, 12);
  return base
    .map((item) => ({
      item,
      score: Math.max(fuzzyScore(query, item.title), fuzzyScore(query, item.meta || item.title)),
    }))
    .filter((entry) => entry.score >= 0)
    .sort((left, right) => right.score - left.score)
    .slice(0, 12)
    .map((entry) => entry.item);
}

function renderPrompt(query) {
  if (!dom.modalList) return;
  promptState.items = promptResults(query);
  promptState.index = Math.min(promptState.index, Math.max(promptState.items.length - 1, 0));
  const list = dom.modalList;
  list.textContent = '';
  if (!promptState.items.length) {
    const empty = document.createElement('li');
    empty.className = 'modal-empty';
    empty.textContent = promptState.mode === 'switcher' ? 'Aucune note.' : 'Aucune commande.';
    list.appendChild(empty);
    return;
  }
  promptState.items.forEach((item, index) => {
    const li = document.createElement('li');
    li.className = `modal-item${index === promptState.index ? ' is-active' : ''}`;
    li.dataset.testid = 'modal-item';
    li.innerHTML = iconUse(item.icon)
      + `<span class="modal-item-title">${esc(item.title)}</span>`
      + (item.meta ? `<span class="modal-item-meta">${esc(item.meta)}</span>` : '')
      + (item.kbd ? `<span class="modal-item-meta">${esc(item.kbd)}</span>` : '');
    li.addEventListener('click', () => runPromptItem(index));
    li.addEventListener('pointermove', () => {
      if (promptState.index !== index) {
        promptState.index = index;
        highlightPrompt();
      }
    });
    list.appendChild(li);
  });
}

function highlightPrompt() {
  if (!dom.modalList) return;
  Array.from(dom.modalList.children).forEach((li, index) => {
    li.classList.toggle('is-active', index === promptState.index);
  });
}

function runPromptItem(index) {
  const item = promptState.items[index];
  if (!item) return;
  closePrompt();
  item.run();
}

function wirePrompt() {
  if (!dom.modalInput || !dom.modalVeil) return;
  dom.modalInput.addEventListener('input', () => {
    promptState.index = 0;
    renderPrompt(dom.modalInput.value);
  });
  dom.modalInput.addEventListener('keydown', (event) => {
    if (event.key === 'ArrowDown') {
      event.preventDefault();
      promptState.index = Math.min(promptState.index + 1, promptState.items.length - 1);
      highlightPrompt();
    } else if (event.key === 'ArrowUp') {
      event.preventDefault();
      promptState.index = Math.max(promptState.index - 1, 0);
      highlightPrompt();
    } else if (event.key === 'Enter') {
      event.preventDefault();
      runPromptItem(promptState.index);
    } else if (event.key === 'Escape') {
      event.preventDefault();
      closePrompt();
    }
  });
  dom.modalVeil.addEventListener('pointerdown', (event) => {
    if (event.target === dom.modalVeil) closePrompt();
  });
}

/* ============================ Vue graphique ============================
   Canvas 2D, simulation force-dirigée (répulsion / ressorts / centrage).
   Les nœuds sont colorés par dossier de premier niveau, comme la vue
   iced. Molette : zoom · glisser fond : pan · glisser nœud : réorganiser
   · clic nœud : ouvrir la note.                                        */

const graphState = {
  running: false,
  raf: 0,
  alpha: 0,
  nodes: [],
  edges: [],
  byPath: new Map(),
  view: { x: 0, y: 0, scale: 1 },
  hover: null,
  drag: null,
  panning: false,
  pointer: null,
  moved: false,
};

const GRAPH_PALETTE = ['#A68AF9', '#4EC9B0', '#E5C07B', '#E06C75', '#61AFEF', '#C678DD', '#98C379', '#E9973F'];

function folderOf(path) {
  const parts = String(path).split('/');
  return parts.length > 1 ? parts[0] : '';
}

function folderColor(path) {
  let h = 5381;
  for (const ch of folderOf(path)) h = ((h << 5) + h + ch.charCodeAt(0)) | 0;
  return GRAPH_PALETTE[(h >>> 0) % GRAPH_PALETTE.length];
}

async function openGraph() {
  if (!dom.graphOverlay) return;
  let payload;
  try {
    payload = await api.graph();
  } catch (error) {
    showError(`graphe indisponible : ${error.message}`);
    return;
  }
  const nodes = (payload && payload.nodes) || [];
  const edges = (payload && payload.edges) || [];
  const radius = Math.max(80, Math.sqrt(Math.max(nodes.length, 1)) * 60);
  graphState.nodes = nodes.map((node, index) => {
    const angle = (index / Math.max(nodes.length, 1)) * Math.PI * 2;
    return {
      path: node.path,
      title: titleOf(node.path),
      links: node.links || 0,
      color: folderColor(node.path),
      x: Math.cos(angle) * radius + (Math.random() - 0.5) * 40,
      y: Math.sin(angle) * radius + (Math.random() - 0.5) * 40,
      vx: 0,
      vy: 0,
    };
  });
  graphState.byPath = new Map(graphState.nodes.map((node) => [node.path, node]));
  graphState.edges = edges
    .map((edge) => ({ a: graphState.byPath.get(edge.from), b: graphState.byPath.get(edge.to) }))
    .filter((edge) => edge.a && edge.b);
  graphState.view = { x: 0, y: 0, scale: 1 };
  graphState.alpha = 0.5;
  graphState.hover = null;
  graphState.drag = null;
  dom.graphOverlay.hidden = false;
  if (!graphState.running) {
    graphState.running = true;
    graphState.raf = window.requestAnimationFrame(graphFrame);
  }
}

function closeGraph() {
  graphState.running = false;
  if (graphState.raf) window.cancelAnimationFrame(graphState.raf);
  graphState.raf = 0;
  if (dom.graphOverlay) dom.graphOverlay.hidden = true;
}

function stepGraphPhysics() {
  const nodes = graphState.nodes;
  const count = nodes.length;
  if (!count) return;
  const a = graphState.alpha;
  const REP = 3200;
  const SPRING = 0.02;
  const LEN = 92;
  // La répulsion est en O(n²) : au-delà de ~400 nœuds on la saute (les
  // ressorts et le centrage suffisent à étaler le graphe).
  if (count <= 400) {
    for (let i = 0; i < count; i += 1) {
      const p = nodes[i];
      for (let j = i + 1; j < count; j += 1) {
        const q = nodes[j];
        let dx = q.x - p.x;
        let dy = q.y - p.y;
        let d2 = dx * dx + dy * dy;
        if (d2 < 1) {
          dx = Math.random() - 0.5;
          dy = Math.random() - 0.5;
          d2 = 1;
        }
        const d = Math.sqrt(d2);
        const f = (REP / (d2 + 40)) * a;
        const fx = (dx / d) * f;
        const fy = (dy / d) * f;
        p.vx -= fx;
        p.vy -= fy;
        q.vx += fx;
        q.vy += fy;
      }
    }
  }
  for (const edge of graphState.edges) {
    const dx = edge.b.x - edge.a.x;
    const dy = edge.b.y - edge.a.y;
    const d = Math.max(Math.sqrt(dx * dx + dy * dy), 1);
    const f = (d - LEN) * SPRING * a;
    const fx = (dx / d) * f;
    const fy = (dy / d) * f;
    edge.a.vx += fx;
    edge.a.vy += fy;
    edge.b.vx -= fx;
    edge.b.vy -= fy;
  }
  for (const node of nodes) {
    node.vx -= node.x * 0.002 * a;
    node.vy -= node.y * 0.002 * a;
    if (graphState.drag === node) {
      node.vx = 0;
      node.vy = 0;
      continue;
    }
    node.vx *= 0.85;
    node.vy *= 0.85;
    node.x += Math.max(-12, Math.min(12, node.vx));
    node.y += Math.max(-12, Math.min(12, node.vy));
  }
}

function graphFrame() {
  if (!graphState.running) return;
  if (graphState.alpha > 0.008 || graphState.drag) {
    stepGraphPhysics();
    graphState.alpha *= 0.994;
    if (graphState.drag) graphState.alpha = Math.max(graphState.alpha, 0.25);
  }
  drawGraph();
  graphState.raf = window.requestAnimationFrame(graphFrame);
}

function drawGraph() {
  const canvas = dom.graphCanvas;
  if (!canvas) return;
  const dpr = window.devicePixelRatio || 1;
  const w = canvas.clientWidth;
  const h = canvas.clientHeight;
  if (canvas.width !== Math.round(w * dpr) || canvas.height !== Math.round(h * dpr)) {
    canvas.width = Math.round(w * dpr);
    canvas.height = Math.round(h * dpr);
  }
  const ctx = canvas.getContext('2d');
  if (!ctx) return;
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  ctx.clearRect(0, 0, w, h);
  const view = graphState.view;
  const toScreenX = (node) => (node.x - view.x) * view.scale + w / 2;
  const toScreenY = (node) => (node.y - view.y) * view.scale + h / 2;

  ctx.strokeStyle = 'rgba(140, 140, 160, 0.28)';
  ctx.lineWidth = 1;
  ctx.beginPath();
  for (const edge of graphState.edges) {
    ctx.moveTo(toScreenX(edge.a), toScreenY(edge.a));
    ctx.lineTo(toScreenX(edge.b), toScreenY(edge.b));
  }
  ctx.stroke();

  const labelColor = state.theme === 'light' ? 'rgba(60, 60, 60, 0.85)' : 'rgba(178, 178, 178, 0.9)';
  const showLabels = view.scale > 0.45 || graphState.nodes.length <= 40;
  ctx.textAlign = 'center';
  ctx.font = '11px Inter, system-ui, sans-serif';
  for (const node of graphState.nodes) {
    const sx = toScreenX(node);
    const sy = toScreenY(node);
    const r = 3.5 + Math.sqrt(node.links) * 1.7;
    if (sx < -60 || sx > w + 60 || sy < -60 || sy > h + 60) continue;
    ctx.beginPath();
    ctx.arc(sx, sy, r, 0, Math.PI * 2);
    ctx.fillStyle = node.color;
    ctx.globalAlpha = 0.9;
    ctx.fill();
    ctx.globalAlpha = 1;
    if (node === graphState.hover) {
      ctx.strokeStyle = state.theme === 'light' ? '#222222' : '#FAFAFA';
      ctx.lineWidth = 1.5;
      ctx.stroke();
    }
    if (showLabels || node === graphState.hover) {
      ctx.fillStyle = labelColor;
      ctx.fillText(node.title, sx, sy + r + 13);
    }
  }
}

function graphNodeAt(mx, my) {
  const canvas = dom.graphCanvas;
  if (!canvas) return null;
  const w = canvas.clientWidth;
  const h = canvas.clientHeight;
  const view = graphState.view;
  let best = null;
  let bestD = Infinity;
  for (const node of graphState.nodes) {
    const sx = (node.x - view.x) * view.scale + w / 2;
    const sy = (node.y - view.y) * view.scale + h / 2;
    const r = 3.5 + Math.sqrt(node.links) * 1.7 + 4;
    const d = (mx - sx) * (mx - sx) + (my - sy) * (my - sy);
    if (d <= r * r && d < bestD) {
      best = node;
      bestD = d;
    }
  }
  return best;
}

function wireGraph() {
  const canvas = dom.graphCanvas;
  if (!canvas) return;
  if (dom.graphClose) dom.graphClose.addEventListener('click', closeGraph);

  canvas.addEventListener('pointerdown', (event) => {
    canvas.setPointerCapture(event.pointerId);
    const rect = canvas.getBoundingClientRect();
    graphState.moved = false;
    graphState.pointer = { x: event.clientX, y: event.clientY };
    const node = graphNodeAt(event.clientX - rect.left, event.clientY - rect.top);
    if (node) graphState.drag = node;
    else graphState.panning = true;
    canvas.classList.add('is-dragging');
  });

  canvas.addEventListener('pointermove', (event) => {
    const rect = canvas.getBoundingClientRect();
    if (graphState.drag) {
      graphState.moved = true;
      graphState.drag.x += (event.clientX - graphState.pointer.x) / graphState.view.scale;
      graphState.drag.y += (event.clientY - graphState.pointer.y) / graphState.view.scale;
      graphState.pointer = { x: event.clientX, y: event.clientY };
      graphState.alpha = Math.max(graphState.alpha, 0.25);
    } else if (graphState.panning) {
      if (Math.abs(event.clientX - graphState.pointer.x) + Math.abs(event.clientY - graphState.pointer.y) > 3) {
        graphState.moved = true;
      }
      graphState.view.x -= (event.clientX - graphState.pointer.x) / graphState.view.scale;
      graphState.view.y -= (event.clientY - graphState.pointer.y) / graphState.view.scale;
      graphState.pointer = { x: event.clientX, y: event.clientY };
    } else {
      const node = graphNodeAt(event.clientX - rect.left, event.clientY - rect.top);
      if (node !== graphState.hover) {
        graphState.hover = node;
        canvas.style.cursor = node ? 'pointer' : 'grab';
      }
    }
  });

  canvas.addEventListener('pointerup', (event) => {
    if (!graphState.moved) {
      const rect = canvas.getBoundingClientRect();
      const node = graphNodeAt(event.clientX - rect.left, event.clientY - rect.top);
      if (node) {
        closeGraph();
        openNote(node.path);
        return;
      }
    }
    graphState.drag = null;
    graphState.panning = false;
    canvas.classList.remove('is-dragging');
  });

  canvas.addEventListener('pointercancel', () => {
    graphState.drag = null;
    graphState.panning = false;
    if (dom.graphCanvas) dom.graphCanvas.classList.remove('is-dragging');
  });

  canvas.addEventListener('wheel', (event) => {
    event.preventDefault();
    const rect = canvas.getBoundingClientRect();
    const view = graphState.view;
    const mx = event.clientX - rect.left - canvas.clientWidth / 2;
    const my = event.clientY - rect.top - canvas.clientHeight / 2;
    const wx = view.x + mx / view.scale;
    const wy = view.y + my / view.scale;
    const factor = Math.exp(-event.deltaY * 0.0016);
    view.scale = Math.min(4, Math.max(0.12, view.scale * factor));
    view.x = wx - mx / view.scale;
    view.y = wy - my / view.scale;
  }, { passive: false });
}

/* ============================ Calendrier / note quotidienne ============================ */

const CAL_MONTHS = ['janvier', 'février', 'mars', 'avril', 'mai', 'juin', 'juillet', 'août', 'septembre', 'octobre', 'novembre', 'décembre'];
const CAL_WEEKDAYS = ['L', 'M', 'M', 'J', 'V', 'S', 'D'];

function pad2(value) {
  return String(value).padStart(2, '0');
}

function isoDate(year, month, day) {
  return `${year}-${pad2(month + 1)}-${pad2(day)}`;
}

function todayIso() {
  const now = new Date();
  return isoDate(now.getFullYear(), now.getMonth(), now.getDate());
}

state.calMonth = (() => {
  const now = new Date();
  return { y: now.getFullYear(), m: now.getMonth() };
})();

function dailyNotePath(dateStr) {
  const wanted = `${dateStr}.md`;
  const exact = state.files.find((file) => file.path === wanted);
  if (exact) return exact.path;
  const byName = state.files.find((file) => basename(file.path) === wanted);
  return byName ? byName.path : wanted;
}

function renderCalendar() {
  const grid = dom.calGrid;
  if (!grid) return;
  const { y, m } = state.calMonth;
  if (dom.calTitle) dom.calTitle.textContent = `${CAL_MONTHS[m]} ${y}`;
  grid.textContent = '';
  for (const weekday of CAL_WEEKDAYS) {
    const cell = document.createElement('div');
    cell.className = 'cal-weekday';
    cell.textContent = weekday;
    grid.appendChild(cell);
  }
  const startOffset = (new Date(y, m, 1).getDay() + 6) % 7;
  const daysInMonth = new Date(y, m + 1, 0).getDate();
  const today = todayIso();
  const known = new Set(state.files.map((file) => basename(file.path)));
  for (let day = 1; day <= daysInMonth; day += 1) {
    const dateStr = isoDate(y, m, day);
    const cell = document.createElement('div');
    cell.className = 'calendar-cell';
    cell.dataset.testid = 'calendar-cell';
    cell.dataset.date = dateStr;
    cell.textContent = String(day);
    if (dateStr === today) cell.classList.add('is-today');
    if (known.has(`${dateStr}.md`)) {
      cell.classList.add('has-note');
      const dot = document.createElement('span');
      dot.className = 'cal-dot';
      cell.appendChild(dot);
    }
    cell.addEventListener('click', () => openDaily(dateStr));
    grid.appendChild(cell);
  }
  for (let i = 0; i < startOffset; i += 1) {
    const cell = document.createElement('div');
    cell.className = 'calendar-cell';
    grid.insertBefore(cell, grid.children[CAL_WEEKDAYS.length]);
  }
}

function shiftCalendar(delta) {
  let { y, m } = state.calMonth;
  m += delta;
  if (m < 0) { m = 11; y -= 1; }
  else if (m > 11) { m = 0; y += 1; }
  state.calMonth = { y, m };
  renderCalendar();
}

async function openDaily(dateStr) {
  const exists = state.files.some((file) => basename(file.path) === `${dateStr}.md`);
  const path = dailyNotePath(dateStr);
  if (exists) {
    openNote(path);
    return;
  }
  try {
    await api.write(path, `# ${dateStr}\n\n`);
    await refreshFiles();
    await openNote(path);
  } catch (error) {
    showError(`note quotidienne impossible : ${error.message}`);
  }
}

function openDailyNote() {
  openDaily(todayIso());
}

/* ============================ Raccourcis globaux ============================ */

function wireGlobalKeys() {
  document.addEventListener('keydown', (event) => {
    if (event.key === 'Escape') {
      if (promptState.mode) {
        closePrompt();
        event.preventDefault();
      } else if (graphState.running) {
        closeGraph();
        event.preventDefault();
      }
      return;
    }
    if (!(event.ctrlKey || event.metaKey) || event.altKey) return;
    const key = event.key.toLowerCase();
    if (key === 'k') {
      event.preventDefault();
      openPrompt('switcher');
    } else if (key === 'p') {
      event.preventDefault();
      openPrompt('palette');
    } else if (key === 'g') {
      event.preventDefault();
      if (graphState.running) closeGraph();
      else openGraph();
    } else if (key === 'n' && !event.shiftKey) {
      event.preventDefault();
      createNote();
    } else if (key === 'f' && event.shiftKey) {
      event.preventDefault();
      focusGlobalSearch();
    }
  });
}

/* ============================ Ruban, navigation, crochets divers ============================ */

function wireExtras() {
  if (dom.ribbonQuick) dom.ribbonQuick.addEventListener('click', () => openPrompt('switcher'));
  if (dom.ribbonGraph) dom.ribbonGraph.addEventListener('click', () => {
    if (graphState.running) closeGraph();
    else openGraph();
  });
  if (dom.ribbonCommand) dom.ribbonCommand.addEventListener('click', () => openPrompt('palette'));
  if (dom.ribbonDaily) dom.ribbonDaily.addEventListener('click', openDailyNote);
  if (dom.ribbonCanvas) dom.ribbonCanvas.addEventListener('click', () => showError('Canvas : à venir dans le front web.'));
  if (dom.ribbonTemplate) dom.ribbonTemplate.addEventListener('click', () => showError('Modèles : à venir dans le front web.'));
  if (dom.ribbonBase) dom.ribbonBase.addEventListener('click', () => showError('Bases : à venir dans le front web.'));
  if (dom.navBack) dom.navBack.addEventListener('click', () => goHistory(-1));
  if (dom.navForward) dom.navForward.addEventListener('click', () => goHistory(1));
  if (dom.toggleLeftDock) {
    dom.toggleLeftDock.addEventListener('click', () => {
      document.querySelector('.app').classList.toggle('no-left-dock');
    });
  }
  if (dom.statusTheme) {
    dom.statusTheme.classList.add('is-clickable');
    dom.statusTheme.title = 'Basculer le thème';
    dom.statusTheme.addEventListener('click', toggleTheme);
  }
  if (dom.calPrev) dom.calPrev.addEventListener('click', () => shiftCalendar(-1));
  if (dom.calNext) dom.calNext.addEventListener('click', () => shiftCalendar(1));
}

/* ============================ Initialisation des ajouts ============================ */

document.addEventListener('DOMContentLoaded', () => {
  applyTheme(state.theme);
  wirePrompt();
  wireGraph();
  wireExtras();
  wireGlobalKeys();
  updateNavButtons();
});
