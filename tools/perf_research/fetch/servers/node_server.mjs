// HTTP server using Node's built-in fetch-compatible primitives via node:http.
// Uses standard node:http for parity with Deno.serve (both are native HTTP servers
// without an undici-style Request/Response wrapper). Run with Node 22+.
import http from "node:http";

const port = Number(
  process.argv.find((a) => a.startsWith("--port="))?.slice(7),
) || 8081;

const helloBytes = Buffer.from(JSON.stringify({ ok: true, msg: "hello" }));

const server = http.createServer(async (req, res) => {
  switch (req.url) {
    case "/hello": {
      res.setHeader("content-type", "application/json");
      res.end(helloBytes);
      return;
    }
    case "/echo": {
      const chunks = [];
      for await (const c of req) chunks.push(c);
      const body = JSON.parse(Buffer.concat(chunks).toString("utf8"));
      res.setHeader("content-type", "application/json");
      res.end(JSON.stringify(body));
      return;
    }
    case "/echo-bytes": {
      const chunks = [];
      for await (const c of req) chunks.push(c);
      res.setHeader("content-type", "application/octet-stream");
      res.end(Buffer.concat(chunks));
      return;
    }
    case "/headers": {
      let count = 0;
      for (const _ of Object.entries(req.headers)) count++;
      res.end(`${count}`);
      return;
    }
    default:
      res.statusCode = 404;
      res.end("not found");
  }
});
server.listen(port);
