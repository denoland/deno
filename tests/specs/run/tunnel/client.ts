let serveAddr;

Deno.serve({
  tunnel: true,
  onListen(addr) {
    serveAddr = addr;
  },
}, async (req, info) => {
  const headers = Object.fromEntries(req.headers);
  return Response.json({
    method: req.method,
    url: req.url,
    headers,
    remoteAddr: info.remoteAddr,
    serveAddr,
  });
});
