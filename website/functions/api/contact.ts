// POST /api/contact: hand the form to the noughtpad-contact Worker, which sends the email.
// Pages Functions can't use a send_email binding directly, so this is a thin pass-through.
interface Env {
  CONTACT: { fetch(request: Request): Promise<Response> };
}

export const onRequestPost = async ({ request, env }: { request: Request; env: Env }) => env.CONTACT.fetch(request);
