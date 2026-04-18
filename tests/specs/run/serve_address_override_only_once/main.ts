const server1 = Deno.serve({
  port: 9000, // This should be overridden to 9001 by DENO_SERVE_ADDRESS
  onListen({ port, hostname }) {
    console.log(`Server 1 listening on ${hostname}:${port}`);
  },
}, () => new Response("Server 1"));
const server2 = Deno.serve({
  port: 9002, // This should NOT be overridden, should stay 9002
  onListen({ port, hostname }) {
    console.log(`Server 2 listening on ${hostname}:${port}`);
  },
}, () => new Response("Server 2"));

// Verify both servers are accessible on expected ports
const resp1 = await fetch("http://localhost:9001/");
console.log(`Server 1 response: ${await resp1.text()}`);

const resp2 = await fetch("http://localhost:9002/");
console.log(`Server 2 response: ${await resp2.text()}`);

await server1.shutdown();
await server2.shutdown();
