// A `default` entry never scrubs, so it must not suppress the legacy
// X_DENO_FETCH_TOKEN handling: the anti-spoofing scrub has to keep running,
// and the operator's own value keeps precedence over the policy fallback.

const server = Deno.serve({ port: 0, onListen() {} }, (req: Request) => {
  return Response.json({
    "x-deno-fetch-token": req.headers.get("x-deno-fetch-token"),
  });
});

const resp = await fetch(`http://localhost:${server.addr.port}/`, {
  headers: { "x-deno-fetch-token": "spoofed-by-user-code" },
});
const { "x-deno-fetch-token": token } = await resp.json();
console.log("token:", token);

await server.shutdown();
