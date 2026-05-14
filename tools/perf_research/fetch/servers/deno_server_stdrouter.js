// HTTP server using Deno.serve, with Request/Response semantics (Web Fetch surface).
// This stresses the Web Fetch Request/Response/Headers JS layer on the server side,
// in contrast to deno_server.js which uses the same surface but is the same path.
// Kept for future variants (e.g. router-style).
const port = Number(Deno.args.find((a) => a.startsWith("--port="))?.slice(7)) ||
  8083;

Deno.serve({ port, onListen: () => {} }, (req) => {
  const url = new URL(req.url);
  const out = new Headers();
  out.set("x-original-method", req.method);
  out.set("x-original-path", url.pathname);
  for (const [k, v] of req.headers) {
    out.append(`x-echo-${k}`, v);
  }
  return new Response("ok", { headers: out });
});
