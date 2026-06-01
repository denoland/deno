import * as http from "node:http";

const SOCK = "/tmp/deno-http-override-only.sock";
try {
  Deno.removeSync(SOCK);
} catch { /* ignore */ }

// Non-duplicate unix override: the user's listen(9999) should be
// effectively ignored, and the server should only be reachable over
// the unix socket.
const server = http.createServer((_req, res) => res.end("unix-only"));
server.listen(9999, "127.0.0.1", async () => {
  // Ensure the TCP address is NOT reachable. We expect fetch to fail.
  let tcpReachable = false;
  try {
    const r = await fetch("http://127.0.0.1:9999/");
    await r.text();
    tcpReachable = true;
  } catch {
    tcpReachable = false;
  }
  console.log(`tcp reachable: ${tcpReachable}`);

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
