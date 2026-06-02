import * as https from "node:https";
import { readFileSync } from "node:fs";

const cert = readFileSync(
  new URL("../../../testdata/tls/localhost.crt", import.meta.url),
);
const key = readFileSync(
  new URL("../../../testdata/tls/localhost.key", import.meta.url),
);

// https.Server should honor the address override the same way
// http.Server does. The override channel is cleartext HTTP -- that
// matches the Deno Deploy use case where the override is a trusted
// vsock/unix control plane.
const SOCK = "/tmp/deno-https-override-test.sock";
try {
  Deno.removeSync(SOCK);
} catch { /* ignore */ }

const server = https.createServer(
  { cert, key },
  (_req, res) => res.end("ok"),
);
server.listen(9405, "127.0.0.1", async () => {
  const r1 = await fetch("https://127.0.0.1:9405/");
  console.log(`https: ${await r1.text()}`);

  // Plain HTTP over unix socket reaches the server via the override
  // listener.
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
  const body = resp.split("\r\n\r\n")[1] ?? "";
  console.log(`unix body: ${body}`);

  server.close();
  try {
    Deno.removeSync(SOCK);
  } catch { /* ignore */ }
});
