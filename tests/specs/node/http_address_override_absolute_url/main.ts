// Connections accepted via the DENO_SERVE_ADDRESS override carry
// absolute-form request targets; node:http servers must see origin-form
// req.url.
import { createServer } from "node:http";

const server = createServer((req, res) => {
  res.setHeader("content-type", "application/json");
  res.end(JSON.stringify(req.url));
});
server.listen(0, async () => {
  const conn = await Deno.connect({ hostname: "127.0.0.1", port: 12470 });
  await conn.write(
    new TextEncoder().encode(
      "GET https://app.example/some/path?q=1 HTTP/1.1\r\n" +
        "Host: app.example\r\nConnection: close\r\n\r\n",
    ),
  );
  const buf = new Uint8Array(65536);
  let text = "";
  while (true) {
    const n = await conn.read(buf);
    if (n === null) break;
    text += new TextDecoder().decode(buf.subarray(0, n));
  }
  conn.close();
  const body = text.slice(text.indexOf("\r\n\r\n") + 4);
  console.log("node req.url:", body);
  server.close(() => Deno.exit(0));
});
