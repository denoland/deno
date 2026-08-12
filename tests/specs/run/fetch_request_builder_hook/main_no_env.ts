// Test that without env vars, no extra headers are injected.

const server = Deno.serve({ port: 0 }, (req: Request) => {
  return Response.json({
    "x-deno-fetch-token": req.headers.get("x-deno-fetch-token"),
    "cdn-loop": req.headers.get("cdn-loop"),
  });
});

const addr = server.addr;
const url = `http://localhost:${addr.port}/`;

const resp = await fetch(url);
const headers = await resp.json();
console.log("no fetch token:", headers["x-deno-fetch-token"] === null);
console.log("no cdn-loop:", headers["cdn-loop"] === null);

await server.shutdown();
