// The legacy egress env vars are resolved at worker construction, before user
// code runs, so `Deno.env.set` cannot influence what the request builder hook
// injects. Reading them lazily on the first fetch instead would let user code
// pick the values and defeat the anti-spoofing scrub.

Deno.env.set("X_DENO_FETCH_TOKEN", "spoofed-token");
Deno.env.set("CDN_LOOP", "spoofed-loop");

const server = Deno.serve({ port: 0, onListen() {} }, (req: Request) => {
  return Response.json({
    "x-deno-fetch-token": req.headers.get("x-deno-fetch-token"),
    "cdn-loop": req.headers.get("cdn-loop"),
  });
});

// The first fetch of the process: if the values were resolved lazily, this is
// the call that would latch the spoofed ones.
const resp = await fetch(`http://localhost:${server.addr.port}/`);
const headers = await resp.json();
console.log("x-deno-fetch-token:", headers["x-deno-fetch-token"]);
console.log("cdn-loop:", headers["cdn-loop"]);

await server.shutdown();
