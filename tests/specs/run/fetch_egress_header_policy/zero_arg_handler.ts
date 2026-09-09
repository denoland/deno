// A zero-argument handler normally opts out of materializing the inbound
// request headers. `forward` needs them, so the capture must work regardless
// of the handler's arity - otherwise the hop count depends on how the handler
// happens to be written.

const echo = Deno.serve({ port: 0, onListen() {} }, (req: Request) => {
  return Response.json({ "cdn-loop": req.headers.get("cdn-loop") });
});

const proxyFor = (handlerTakesRequest: boolean) => {
  const fetchEcho = async () => {
    const resp = await fetch(`http://localhost:${echo.addr.port}/`);
    return Response.json(await resp.json());
  };
  return handlerTakesRequest
    ? Deno.serve({ port: 0, onListen() {} }, (_req: Request) => fetchEcho())
    : Deno.serve({ port: 0, onListen() {} }, () => fetchEcho());
};

// Printing the value rather than a comparison keeps a regression readable: a
// handler that captured nothing shows up as a single entry instead of two.
for (const takesRequest of [false, true]) {
  const proxy = proxyFor(takesRequest);
  const resp = await fetch(`http://localhost:${proxy.addr.port}/`);
  const { "cdn-loop": value } = await resp.json();
  console.log(`${takesRequest ? "one-arg" : "zero-arg"} handler:`, value);
  await proxy.shutdown();
}

await echo.shutdown();
