// Build a mock seed from the sample notebook: [created, content, labels, sourceTitle, title]
const fs = require('fs'), path = require('path');
const root = path.resolve(__dirname, '../../../samples/vault');
const out = [];
const walk = d => fs.readdirSync(d, { withFileTypes: true }).forEach(e => {
  const p = path.join(d, e.name);
  if (e.isDirectory()) { if (!e.name.startsWith('.')) walk(p); }
  else if (e.name.endsWith('.md')) out.push(p);
});
walk(root);
const seed = out.map(p => {
  let t = fs.readFileSync(p, 'utf8'); let fm = '';
  const m = t.match(/^---\n([\s\S]*?)\n---\n?/);
  if (m) { fm = m[1]; t = t.slice(m[0].length); }
  const get = k => { const r = fm.match(new RegExp('^' + k + ':\\s*(.+)$', 'm')); return r ? r[1].trim().replace(/^["']|["']$/g, '') : null; };
  const labels = (fm.match(/^labels:\s*\[(.*)\]/m)?.[1] ?? '').split(',').map(s => s.trim().replace(/^["']|["']$/g, '')).filter(Boolean);
  const src = fm.match(/^source:\s*\n\s+title:\s*(.+)$/m)?.[1]?.trim().replace(/^["']|["']$/g, '') ?? null;
  const created = get('created') ?? '2026-05-15T09:00:00+05:30';
  return [created, t.trim(), labels, src, get('title')];
}).sort((a, b) => a[0].localeCompare(b[0]));
fs.writeFileSync(path.join(__dirname, 'seed.json'), JSON.stringify(seed));
console.log(seed.length, seed.slice(0, 3).map(s => [s[0], s[2], s[3], s[4]]));
