let serveAddr;

const client = Deno.createHttpClient({
  proxy: { transport: "tunnel", kind: "agent" },
});

Deno.serve({
  onListen(addr) {
    serveAddr = addr;
  },
}, async (req, info) => {
  const headers = Object.fromEntries(req.headers);

  await fetch("http://meow.com", { client });

  return Response.json({
    method: req.method,
    url: req.url,
    headers,
    remoteAddr: info.remoteAddr,
    serveAddr,
  });
});
