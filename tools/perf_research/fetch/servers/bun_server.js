// HTTP server using Bun.serve.
// Run: bun servers/bun_server.js --port=8082
const port = Number(
  Bun.argv.find((a) => a.startsWith("--port="))?.slice(7),
) || 8082;

const helloBytes = new TextEncoder().encode(
  JSON.stringify({ ok: true, msg: "hello" }),
);

Bun.serve({
  port,
  async fetch(req) {
    const url = new URL(req.url);
    switch (url.pathname) {
      case "/hello":
        return new Response(helloBytes, {
          headers: { "content-type": "application/json" },
        });
      case "/echo": {
        const body = await req.json();
        return new Response(JSON.stringify(body), {
          headers: { "content-type": "application/json" },
        });
      }
      case "/echo-bytes": {
        const buf = await req.arrayBuffer();
        return new Response(buf, {
          headers: { "content-type": "application/octet-stream" },
        });
      }
      case "/headers": {
        let count = 0;
        for (const _h of req.headers) count++;
        return new Response(`${count}`);
      }
      case "/bigbody": {
        const buf = new Uint8Array(1 << 20).fill(0x41);
        return new Response(buf, {
          headers: { "content-type": "application/octet-stream" },
        });
      }
      default:
        return new Response("not found", { status: 404 });
    }
  },
});
