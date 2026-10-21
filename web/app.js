/* ---------------------------------------------------------------------------
   obsidian-rs — comportement du front HTML.
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
    (() => { try { return window.localStorage.getItem('obsidian-rs:engine'); } catch (_) { return null; } })(),
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
  dom.statusTheme.textContent = 'Sombre';
}

/* ============================ Ouverture d'une note ============================ */

async function openNote(path) {
  if (!path) return;
  try {
    if (!state.contents.has(path)) {
      const content = await api.read(path);
      state.contents.set(path, content);
    }
    state.activePath = path;
    if (!state.openTabs.includes(path)) state.openTabs.push(path);
    state.dirty = false;

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
  } catch (error) {
    showError(`coffre illisible : ${error.message}`);
  }
}

async function boot() {
  try {
    const health = await api.health();
    const root = health && health.root ? health.root : '';
    dom.vaultName.textContent = basename(root) || 'obsidian-rs';
  } catch (error) {
    dom.vaultName.textContent = 'obsidian-rs';
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
  dom.togglePreview.addEventListener('click', () => {
    const previewPane = dom.workspace.querySelector('.preview-pane');
    const hidden = previewPane.style.display === 'none';
    previewPane.style.display = hidden ? 'flex' : 'none';
    dom.splitter.style.display = hidden ? 'block' : 'none';
  });

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
