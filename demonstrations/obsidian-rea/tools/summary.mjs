// Résume compact d'un rapport CDP (JSON) : console, exceptions, journaux,
// chronologie et sonde d'état.
import fs from 'node:fs';

const file = process.argv[2];
const r = JSON.parse(fs.readFileSync(file, 'utf8'));
console.log('## cibles');
for (const t of r.targets || []) console.log(` - ${t.type} ${t.url}${t.closed ? ' [FERMÉE à ' + t.closeTime + ' ms]' : ''}`);
console.log('## chronologie');
for (const e of r.timeline || []) console.log(` - ${e.t} ms : ${e.event}${e.url ? ' ' + e.url : ''}${e.error ? ' [' + e.error + ']' : ''}`);
console.log('## exceptions');
for (const e of r.exceptions || []) console.log(` - ${e.t} ms : ${e.text} | ${(e.description || '').split('\n')[0]} | ${e.url || ''}:${e.line || ''}`);
console.log('## console');
for (const c of (r.console || []).slice(0, 60)) console.log(` - ${c.t} ms [${c.type}] ${String(c.text).slice(0, 400)}`);
console.log('## journaux page');
for (const l of (r.logs || []).slice(0, 40)) console.log(` - ${l.t} ms [${l.level}/${l.source}] ${String(l.text).slice(0, 300)}`);
console.log('## sonde');
for (const p of r.probes || []) {
  console.log(' - ' + p.url);
  console.log('   ' + JSON.stringify(p.probe, null, 2).split('\n').join('\n   '));
  console.log('   capture: ' + (p.screenshot || p.screenshotError || 'aucune'));
}
