// URL credentials are converted into an Authorization header inside
// `op_fetch`, after the JS half of the egress policy has scrubbed caller
// headers. The later construction step must not bypass policy ownership.

const server = Deno.serve({ port: 0, onListen() {} }, (req: Request) => {
  return new Response(req.headers.get("authorization"));
});

const response = await fetch(
  `http://caller:controlled@localhost:${server.addr.port}/`,
);
console.log("authorization:", await response.text());

await server.shutdown();
