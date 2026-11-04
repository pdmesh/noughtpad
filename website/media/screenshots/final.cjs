const { chromium } = require('playwright');
(async () => {
  const b = await chromium.launch();
  const p = await b.newPage({ viewport: { width: 1280, height: 800 }, deviceScaleFactor: 2 });
  await require('./patch.cjs')(p);
  require('fs').mkdirSync(__dirname + '/out', { recursive: true });
  const go = async () => { await p.goto('http://localhost:5173/?llm=ready&sync=on', { waitUntil: 'networkidle' }); await p.waitForTimeout(1200); };
  const nav = async (name) => { await p.getByRole('button', { name, exact: true }).first().click({ force: true }); await p.waitForTimeout(1300); };
  const shot = (n, o = {}) => p.screenshot({ path: `${__dirname}/out/${n}.png`, ...o });
  // Hero: blank page, typing, chrome hidden
  await go();
  await p.mouse.move(640, 500);
  await p.keyboard.type('Ownership might matter more than the company itself.', { delay: 5 });
  await p.keyboard.press('Enter'); await p.keyboard.press('Enter');
  await p.keyboard.type('If one company returns the fund, what decides how much it returns?', { delay: 5 });
  await p.mouse.move(640, 500); await p.waitForTimeout(1500);
  await shot('write');
  // Pin chrome for the rest
  await go(); await p.keyboard.press('Meta+\\'); await p.waitForTimeout(500);
  await nav('All notes'); await shot('all-notes');
  await p.getByText('good company', { exact: false }).first().click(); await p.waitForTimeout(1500);
  await p.keyboard.press('Meta+j'); await p.waitForTimeout(2000); await shot('related');
  await p.keyboard.press('Escape'); await p.waitForTimeout(400);
  await p.keyboard.press('Meta+k'); await p.waitForTimeout(500); await p.keyboard.type('ownership'); await p.waitForTimeout(1200); await shot('search');
  await p.keyboard.press('Escape'); await p.waitForTimeout(400);
  await nav('Journal'); await shot('journal');
  await nav('Ideas over time'); await shot('ideas');
  // Open the first thread
  await p.getByText('notes ·', { exact: false }).first().click(); await p.waitForTimeout(1500); await shot('thread');
  await nav('Chat');
  await p.getByPlaceholder('Ask your notes…').fill('How has my thinking on ownership changed this year?');
  await p.keyboard.press('Enter'); await p.waitForTimeout(9000); await shot('chat');
  await nav('Settings'); await shot('settings');
  await p.keyboard.press('Meta+\\'); await p.waitForTimeout(300);
  console.log((await p.evaluate(() => document.querySelector('main')?.innerText ?? '')).slice(0, 200));
  await b.close();
})().catch(e => { console.error(e); process.exit(1); });
