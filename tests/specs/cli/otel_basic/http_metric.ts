const server = Deno.serve(
  { port: 8080, onListen: () => {} },
  () => new Response("foo"),
);

for (let i = 0; i < 3; i++) {
  await fetch(`http://localhost:8080`);
}

await server.shutdown();
