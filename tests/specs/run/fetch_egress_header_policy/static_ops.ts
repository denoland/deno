// The static ops (`set`/`remove`/`default`) of DENO_EGRESS_HEADER_POLICY
// are enforced on outbound fetch requests.

const server = Deno.serve({ port: 0 }, (req: Request) => {
  return Response.json({
    "user-agent": req.headers.get("user-agent"),
    "x-strip": req.headers.get("x-strip"),
    "x-def": req.headers.get("x-def"),
  });
});

const url = `http://localhost:${server.addr.port}/`;

// `set` overrides the user value, `remove` strips it, `default` respects it.
{
  const resp = await fetch(url, {
    headers: {
      "user-agent": "user/9.9",
      "x-strip": "should-vanish",
      "x-def": "user-value",
    },
  });
  const headers = await resp.json();
  console.log("ua enforced:", headers["user-agent"] === "enforced/1.0");
  console.log("x-strip removed:", headers["x-strip"] === null);
  console.log("x-def respected:", headers["x-def"] === "user-value");
}

// `default` fills in when the header is absent.
{
  const resp = await fetch(url);
  const headers = await resp.json();
  console.log("ua enforced:", headers["user-agent"] === "enforced/1.0");
  console.log("x-def defaulted:", headers["x-def"] === "fallback");
}

await server.shutdown();
