// Pilote CDP minimal (node >= 21, WebSocket global) : se connecte au
// port de débogage d'un Chromium/Electron, capture console, exceptions,
// journaux de page,Target.detached, prend une capture d'écran et sonde
// l'état du renderer. Lecture seule : ne modifie aucune cible.
import fs from 'node:fs';

const port = Number(process.argv[2] || 9222);
const waitMs = Number(process.argv[3] || 15000);
const shotPath = process.argv[4] || '';
let probeExpr = process.argv[5] || 'null';
if (probeExpr.startsWith('@')) probeExpr = fs.readFileSync(probeExpr.slice(1), 'utf8');

class CDP {
  constructor(ws) {
    this.ws = ws;
    this.id = 0;
    this.pending = new Map();
    this.events = [];
    this.closed = false;
  }
  static async connect(url) {
    const ws = new WebSocket(url);
    await new Promise((res, rej) => {
      ws.onopen = res;
      ws.onerror = (e) => rej(new Error('ws error ' + (e.message || '')));
    });
    const c = new CDP(ws);
    ws.onmessage = (ev) => {
      let m;
      try { m = JSON.parse(ev.data); } catch { return; }
      if (m.id && c.pending.has(m.id)) {
        const { res, rej } = c.pending.get(m.id);
        c.pending.delete(m.id);
        if (m.error) rej(new Error(JSON.stringify(m.error)));
        else res(m.result);
      } else if (m.method) {
        c.events.push({ t: Date.now(), method: m.method, params: m.params });
      }
    };
    ws.onclose = () => { c.closed = true; };
    return c;
  }
  send(method, params = {}) {
    const id = ++this.id;
    return new Promise((res, rej) => {
      this.pending.set(id, { res, rej });
      try { this.ws.send(JSON.stringify({ id, method, params })); }
      catch (e) { rej(e); }
      setTimeout(() => {
        if (this.pending.has(id)) {
          this.pending.delete(id);
          rej(new Error('timeout ' + method));
        }
      }, 20000);
    });
  }
  close() { try { this.ws.close(); } catch { /* ignore */ } }
}

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

const t0 = Date.now();
const report = { port, waitMs, targets: [], console: [], exceptions: [], logs: [], timeline: [] };

let list = [];
try {
  list = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json();
} catch (e) {
  console.log(JSON.stringify({ fatal: 'liste des cibles impossible: ' + e.message }, null, 2));
  process.exit(2);
}

const pages = list.filter((t) => t.type === 'page');
report.targets = list.map((t) => ({ id: t.id, type: t.type, url: t.url, title: t.title }));

for (const page of pages) {
  let c;
  try {
    c = await CDP.connect(page.webSocketDebuggerUrl);
  } catch (e) {
    report.timeline.push({ t: Date.now() - t0, event: 'connexion impossible', target: page.url, error: e.message });
    continue;
  }
  try {
    await c.send('Runtime.enable');
    await c.send('Log.enable');
    await c.send('Page.enable');
  } catch (e) {
    report.timeline.push({ t: Date.now() - t0, event: 'enable impossible', error: e.message });
  }
  const targetState = { url: page.url, closed: false, closeTime: null, console: [], exceptions: [], logs: [] };
  const watcher = setInterval(() => {
    fetch(`http://127.0.0.1:${port}/json/list`).then((r) => r.json()).then((l) => {
      const still = l.some((x) => x.id === page.id);
      if (!still && !targetState.closed) {
        targetState.closed = true;
        targetState.closeTime = Date.now() - t0;
        report.timeline.push({ t: targetState.closeTime, event: 'cible disparue', url: page.url });
      }
    }).catch(() => { /* cible morte */ });
  }, 500);

  await sleep(waitMs);

  // Sonde de l'état du renderer
  let probe = null;
  try {
    const r = await c.send('Runtime.evaluate', {
      expression: probeExpr,
      returnByValue: true,
      awaitPromise: true,
    });
    probe = r.result && r.result.value !== undefined ? r.result.value : r.exceptionDetails || null;
  } catch (e) {
    probe = { error: e.message };
  }
  targetState.probe = probe;

  // Capture d'écran de la page
  if (shotPath) {
    try {
      const s = await c.send('Page.captureScreenshot', { format: 'png' });
      fs.writeFileSync(shotPath, Buffer.from(s.data, 'base64'));
      targetState.screenshot = shotPath;
    } catch (e) {
      targetState.screenshotError = e.message;
    }
  }

  for (const ev of c.events) {
    if (ev.method === 'Runtime.consoleAPICalled') {
      const args = (ev.params.args || []).map((a) =>
        a.value !== undefined ? String(a.value) : (a.description || a.type));
      targetState.console.push({ t: ev.t - t0, type: ev.params.type, text: args.join(' ') });
    } else if (ev.method === 'Runtime.exceptionThrown') {
      const d = ev.params.exceptionDetails || {};
      targetState.exceptions.push({
        t: ev.t - t0,
        text: d.text,
        description: d.exception && d.exception.description,
        url: d.url,
        line: d.lineNumber,
      });
    } else if (ev.method === 'Log.entryAdded') {
      const e2 = ev.params.entry || {};
      targetState.logs.push({ t: ev.t - t0, level: e2.level, source: e2.source, text: e2.text, url: e2.url });
    } else if (ev.method === 'Page.loadEventFired' || ev.method === 'Page.domContentEventFired') {
      targetState.console.push({ t: ev.t - t0, type: ev.method, text: page.url });
    }
  }

  clearInterval(watcher);
  c.close();
  report.timeline.push({ t: Date.now() - t0, event: 'cible sondée', url: page.url, closed: targetState.closed });
  report.targets = report.targets.map((x) => (x.id === page.id ? { ...x, closed: targetState.closed, closeTime: targetState.closeTime } : x));
  report.console.push(...targetState.console.map((x) => ({ url: page.url, ...x })));
  report.exceptions.push(...targetState.exceptions.map((x) => ({ url: page.url, ...x })));
  report.logs.push(...targetState.logs.map((x) => ({ url: page.url, ...x })));
  report.probes = report.probes || [];
  report.probes.push({ url: page.url, probe: targetState.probe, screenshot: targetState.screenshot || null, screenshotError: targetState.screenshotError || null });
}

console.log(JSON.stringify(report, null, 2));
