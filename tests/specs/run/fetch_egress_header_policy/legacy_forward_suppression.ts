// When both the legacy CDN_LOOP env var and a policy managing cdn-loop are
// set (the migration configuration), the legacy insert yields entirely to
// the policy — it must not clobber the JS-applied forward/append values.

const echo = Deno.serve({ port: 0 }, (req: Request) => {
  return Response.json({ "cdn-loop": req.headers.get("cdn-loop") });
});

const proxy = Deno.serve({ port: 0 }, async (_req: Request) => {
  const resp = await fetch(`http://localhost:${echo.addr.port}/`);
  return Response.json(await resp.json());
});

const ENTRY = "deno;d=testhash;v=2";

// Top-level fetch: only the policy's appended entry; no "legacy-value".
{
  const resp = await fetch(`http://localhost:${echo.addr.port}/`);
  const { "cdn-loop": value } = await resp.json();
  console.log("append only:", value === ENTRY);
}

// Through one serve hop the entries still accumulate to depth 2.
{
  const resp = await fetch(`http://localhost:${proxy.addr.port}/`);
  const { "cdn-loop": value } = await resp.json();
  console.log("accumulates:", value === `${ENTRY}, ${ENTRY}`);
}

await echo.shutdown();
await proxy.shutdown();
