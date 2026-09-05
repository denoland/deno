// The `forward`/`append` ops accumulate across serve → fetch hops: an
// outbound fetch made while serving a request carries the inbound request's
// cdn-loop values plus the policy's own appended entry, so the entry count
// equals the hop depth.

const echo = Deno.serve({ port: 0 }, (req: Request) => {
  return Response.json({ "cdn-loop": req.headers.get("cdn-loop") });
});

const proxy = Deno.serve({ port: 0 }, async (_req: Request) => {
  // This fetch runs inside the request context: the inbound cdn-loop
  // values are forwarded onto it automatically.
  const resp = await fetch(`http://localhost:${echo.addr.port}/`);
  return Response.json(await resp.json());
});

const ENTRY = "deno;d=testhash;v=2";

// Top-level fetch: no inbound request context, so only the appended entry.
{
  const resp = await fetch(`http://localhost:${echo.addr.port}/`);
  const { "cdn-loop": value } = await resp.json();
  console.log("depth 1:", value === ENTRY);
}

// One serve hop: the proxy's outbound fetch carries the forwarded inbound
// entry plus its own appended entry.
{
  const resp = await fetch(`http://localhost:${proxy.addr.port}/`);
  const { "cdn-loop": value } = await resp.json();
  console.log("depth 2:", value === `${ENTRY}, ${ENTRY}`);
}

// User-supplied values of policy-owned headers are scrubbed.
{
  const resp = await fetch(`http://localhost:${echo.addr.port}/`, {
    headers: { "cdn-loop": "user-junk" },
  });
  const { "cdn-loop": value } = await resp.json();
  console.log("user value scrubbed:", value === ENTRY);
}

await echo.shutdown();
await proxy.shutdown();
