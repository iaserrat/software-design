function respond(body: string, version: string): Response { return new Response(body, version); }
export function ok(): Response { return respond("ok", "HTTP/1.1"); }
export function missing(): Response { return respond("missing", "HTTP/1.1"); }
export function denied(): Response { return respond("denied", "HTTP/1.1"); }
