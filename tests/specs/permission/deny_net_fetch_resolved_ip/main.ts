// Regression test: --deny-net=127.0.0.1 must block fetch() requests whose
// hostname resolves to a denied IP, mirroring the post-resolution check that
// Deno.connect already performs.

const server = Deno.serve(
  { hostname: "0.0.0.0", port: 0, onListen: () => {} },
  () => new Response("hello"),
);
const { port } = server.addr as Deno.NetAddr;

try {
  await fetch(`http://127.0.0.1:${port}/`);
  console.log("FAIL: fetch to 127.0.0.1 was not denied");
} catch {
  console.log("PASS: fetch to 127.0.0.1 denied");
}

try {
  await fetch(`http://localhost:${port}/`);
  console.log(
    "FAIL: fetch to localhost (resolves to denied IP) was not denied",
  );
} catch {
  console.log("PASS: fetch to localhost denied");
}

await server.shutdown();
