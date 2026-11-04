// Shared page setup: sample notebook in the mock, semantic on, sync signed in, Mac platform.
const fs = require('fs');
const seed = fs.readFileSync(__dirname + '/seed.json', 'utf8');
module.exports = async function setup(p) {
  await p.addInitScript(() => Object.defineProperty(Navigator.prototype, 'platform', { get: () => 'MacIntel' }));
  await p.route(/src\/lib\/mockBackend\.ts/, async (r) => {
    const res = await r.fetch(); let b = await res.text();
    const before = b;
    b = b.replace(/const seed = \[[\s\S]*?\n\];/, 'const seed = ' + seed + ';')
      .replace('kindOverride: null\n}));', 'kindOverride: null\n}));\nnotes.forEach((n, i) => { n.title = seed[i][4]; });')
      .replace('let semantic = { state: "off" };', 'let semantic = { state: "ready", notes: 37 };')
      .replace('signedIn: false,\n\taccount: null,\n\tenabled: false,\n\tstate: "off",\n\tlastSync: null', 'signedIn: true,\n\taccount: "you@example.com",\n\tenabled: true,\n\tstate: "idle",\n\tlastSync: Date.now() - 40000')
      .replaceAll('Decaframe', 'Slides');
    if (!b.includes('"ready", notes: 37') || !b.includes('you@example.com')) console.error('mock patch missed: the mock backend changed, update patch.cjs');
    r.fulfill({ response: res, body: b });
  });
};
