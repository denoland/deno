let port;
const server = Deno.serve(
  {
    port: 0,
    onListen: (addr) => {
      port = addr.port;
    },
  },
  () => new Response("foo"),
);

for (let i = 0; i < 3; i++) {
  await fetch(`http://localhost:${port}`);
}

await server.shutdown();
