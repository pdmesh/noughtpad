// Contact form for noughtpad.com. The Pages Function at /api/contact forwards the form POST
// here over a service binding (Pages Functions can't hold a send_email binding). We check
// the fields, drop obvious bots, and send one plain-text email through Email Routing.
// No cookies, no JavaScript on the page, nothing stored.
import { EmailMessage } from "cloudflare:email";

interface Env {
  EMAIL: { send(message: EmailMessage): Promise<void> };
  /** Verified Email Routing destination. Set with `wrangler secret put DESTINATION`. */
  DESTINATION: string;
}

const FROM = "website@noughtpad.com";
const ALLOWED_ORIGINS = [/^https:\/\/(www\.)?noughtpad\.com$/, /^https:\/\/([a-z0-9-]+\.)?noughtpad\.pages\.dev$/, /^http:\/\/localhost:\d+$/];
const EMAIL_RE = /^[^\s@<>"]+@[^\s@<>"]+\.[^\s@<>"]+$/;

const go = (request: Request, path: string) =>
  new Response(null, { status: 303, headers: { Location: new URL(path, request.url).href, "Cache-Control": "no-store" } });

const b64 = (s: string) => {
  const bytes = new TextEncoder().encode(s);
  let bin = "";
  for (const b of bytes) bin += String.fromCharCode(b);
  return btoa(bin);
};
const wrap = (s: string) => s.replace(/.{1,76}/g, "$&\r\n");
const oneLine = (s: string) => s.replace(/[\r\n]+/g, " ").trim();

export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    if (request.method !== "POST") return new Response("Method not allowed", { status: 405, headers: { Allow: "POST" } });

    const origin = request.headers.get("Origin");
    if (origin && !ALLOWED_ORIGINS.some((re) => re.test(origin))) return new Response("Forbidden", { status: 403 });

    let form: FormData;
    try {
      form = await request.formData();
    } catch {
      return go(request, "/contact/problem/");
    }
    const field = (k: string) => String(form.get(k) ?? "").trim();
    const name = oneLine(field("name")).slice(0, 100);
    const email = oneLine(field("email"));
    const message = field("message").replace(/\r\n?/g, "\n");

    // Honeypot: people never see this field, so anything in it is a bot. Pretend it worked.
    if (field("website")) return go(request, "/contact/sent/");

    if (!EMAIL_RE.test(email) || email.length > 200) return go(request, "/contact/problem/");
    if (message.length < 2 || message.length > 5000) return go(request, "/contact/problem/");
    // Link-stuffed messages are almost always spam.
    if ((message.match(/https?:\/\//gi) ?? []).length > 4) return go(request, "/contact/sent/");

    const subject = `Noughtpad contact: ${name || email}`.slice(0, 150);
    const body = `From: ${name || "(no name given)"} <${email}>\n\n${message}\n`;
    const raw = [
      `From: "Noughtpad website" <${FROM}>`,
      `To: <${env.DESTINATION}>`,
      `Reply-To: ${email}`,
      `Subject: =?UTF-8?B?${b64(subject)}?=`,
      `Date: ${new Date().toUTCString()}`,
      `Message-ID: <${crypto.randomUUID()}@noughtpad.com>`,
      "MIME-Version: 1.0",
      "Content-Type: text/plain; charset=utf-8",
      "Content-Transfer-Encoding: base64",
      "",
      wrap(b64(body.replace(/\n/g, "\r\n"))),
    ].join("\r\n");

    try {
      await env.EMAIL.send(new EmailMessage(FROM, env.DESTINATION, raw));
    } catch (err) {
      console.error("send failed", err);
      return go(request, "/contact/problem/");
    }
    return go(request, "/contact/sent/");
  },
};
