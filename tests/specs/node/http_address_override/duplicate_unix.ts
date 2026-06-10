import * as http from "node:http";

const SOCK = "/tmp/deno-http-override-test.sock";
try {
  Deno.removeSync(SOCK);
} catch { /* ignore */ }

// Duplicate mode with a unix-socket override: the server should answer
// on both the user's TCP port and the unix socket.
const server = http.createServer((_req, res) => res.end("ok"));
server.listen(9404, "127.0.0.1", async () => {
  const r1 = await fetch("http://127.0.0.1:9404/");
  console.log(`tcp: ${await r1.text()}`);

  // Send a handcrafted HTTP request over the unix socket. Deno.connect
  // gives us a raw duplex, which exercises the override listener's
  // full data path.
  const conn = await Deno.connect({ transport: "unix", path: SOCK });
  const req = "GET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n";
  await conn.write(new TextEncoder().encode(req));
  const parts: Uint8Array[] = [];
  const buf = new Uint8Array(1024);
  while (true) {
    const n = await conn.read(buf);
    if (n === null) break;
    parts.push(buf.slice(0, n));
  }
  conn.close();
  const resp = new TextDecoder().decode(
    new Uint8Array(parts.flatMap((p) => [...p])),
  );
  // Just extract the body after the header/body separator.
  const body = resp.split("\r\n\r\n")[1] ?? "";
  console.log(`unix body: ${body}`);

  server.close();
  try {
    Deno.removeSync(SOCK);
  } catch { /* ignore */ }
});
