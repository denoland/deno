// The policy applies after the legacy CDN_LOOP / X_DENO_FETCH_TOKEN env
// vars and wins for headers it names; legacy vars it does not name still
// apply unchanged.

const server = Deno.serve({ port: 0 }, (req: Request) => {
  return Response.json({
    "cdn-loop": req.headers.get("cdn-loop"),
    "x-deno-fetch-token": req.headers.get("x-deno-fetch-token"),
  });
});

const resp = await fetch(`http://localhost:${server.addr.port}/`);
const headers = await resp.json();
console.log("policy wins:", headers["cdn-loop"] === "policy-wins");
console.log(
  "legacy token kept:",
  headers["x-deno-fetch-token"] === "legacy-token",
);

await server.shutdown();
