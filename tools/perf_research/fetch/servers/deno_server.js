// HTTP server using Deno.serve. JSON echo / simple text routes.
// Run: deno run -A --no-prompt servers/deno_server.js [--port=8080]
const port = Number(Deno.args.find((a) => a.startsWith("--port="))?.slice(7)) ||
  8080;

const helloBytes = new TextEncoder().encode(
  JSON.stringify({ ok: true, msg: "hello" }),
);

Deno.serve({ port, onListen: () => {} }, async (req) => {
  const url = new URL(req.url);
  switch (url.pathname) {
    case "/hello": {
      return new Response(helloBytes, {
        headers: { "content-type": "application/json" },
      });
    }
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
      // Force consumption of all headers
      let count = 0;
      for (const _h of req.headers) count++;
      return new Response(`${count}`);
    }
    case "/bigbody": {
      // 1 MB ASCII body. Server allocates a fresh buffer each request to
      // exercise the response body construction path.
      const buf = new Uint8Array(1 << 20).fill(0x41);
      return new Response(buf, {
        headers: { "content-type": "application/octet-stream" },
      });
    }
    default:
      return new Response("not found", { status: 404 });
  }
});
