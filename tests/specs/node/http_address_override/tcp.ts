import * as http from "node:http";

// With DENO_SERVE_ADDRESS=tcp:127.0.0.1:9401 the override should replace
// the user-supplied port (9400) with 9401. The second server should
// bind to its own address since the override is consumed only once.
const server1 = http.createServer((_req, res) => res.end("one"));
server1.listen(9400, "127.0.0.1", async () => {
  const addr1 = server1.address() as { port: number; address: string };
  console.log(`server1 bound to ${addr1.address}:${addr1.port}`);

  const server2 = http.createServer((_req, res) => res.end("two"));
  server2.listen(9403, "127.0.0.1", async () => {
    const addr2 = server2.address() as { port: number; address: string };
    console.log(`server2 bound to ${addr2.address}:${addr2.port}`);

    const r1 = await fetch("http://127.0.0.1:9401/");
    console.log(`server1 via 9401: ${await r1.text()}`);

    const r2 = await fetch("http://127.0.0.1:9403/");
    console.log(`server2 via 9403: ${await r2.text()}`);

    server1.close();
    server2.close();
  });
});
