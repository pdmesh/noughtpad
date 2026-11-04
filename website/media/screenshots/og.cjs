const { chromium } = require('playwright');
const fs = require('fs');
(async () => {
  const b = await chromium.launch();
  const p = await b.newPage({ viewport: { width: 1200, height: 630 }, deviceScaleFactor: 1 });
  const shot = 'data:image/png;base64,' + fs.readFileSync(__dirname + '/out/related.png').toString('base64');
  const icon = fs.readFileSync(__dirname + '/../../../assets/icon.svg', 'utf8').replace('<svg', '<svg width="64" height="64"');
  await p.setContent(`<html><body style="margin:0;width:1200px;height:630px;background:#faf8f4;font-family:system-ui,sans-serif;overflow:hidden;position:relative">
    <div style="position:absolute;left:72px;top:96px;width:470px">
      <div style="display:flex;align-items:center;gap:16px">${icon}<span style="font-size:44px;font-weight:700;color:#23262d;letter-spacing:-0.02em">Noughtpad</span></div>
      <div style="margin-top:48px;font-size:56px;line-height:1.1;font-weight:700;color:#23262d;letter-spacing:-0.03em">A blank page that quietly remembers.</div>
      <div style="margin-top:28px;font-size:26px;line-height:1.4;color:#5f6470">Plain Markdown notes on your computer. Related notes and ideas over time, found for you.</div>
    </div>
    <div style="position:absolute;left:600px;top:70px;width:900px;height:562px;border-radius:16px;overflow:hidden;border:1px solid #e6e1d6;box-shadow:0 20px 60px rgba(35,38,45,.18)">
      <img src="${shot}" style="width:1240px;margin-left:-560px;margin-top:-20px;display:block">
    </div></body></html>`);
  await p.screenshot({ path: __dirname + '/../../public/img/og.png' });
  await b.close();
})();
