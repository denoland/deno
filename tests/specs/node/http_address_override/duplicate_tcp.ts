import * as http from "node:http";

// With DENO_SERVE_ADDRESS=duplicate,tcp:127.0.0.1:9402 the server should
// answer on BOTH the user's address AND the override address.
const server = http.createServer((_req, res) => res.end("hello"));
server.listen(9400, "127.0.0.1", async () => {
  const addr = server.address() as { port: number; address: string };
  console.log(`bound to ${addr.address}:${addr.port}`);

  const r1 = await fetch("http://127.0.0.1:9400/");
  console.log(`via user port: ${await r1.text()}`);

  const r2 = await fetch("http://127.0.0.1:9402/");
  console.log(`via override port: ${await r2.text()}`);

  server.close();
});
