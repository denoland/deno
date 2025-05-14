Deno.serve({
  handler(req) {
    console.log(req.headers);
    Deno.exit(0);
  },
  onListen(addr) {
    const url = `http://localhost:${addr.port}/`;
    fetch(url);
  },
  port: 0,
});
