// Regression test for https://github.com/denoland/deno/issues/31955
// Fetch should retry on stale pooled keep-alive connections when the
// server is shut down and restarted between requests.

for (let round = 1; round <= 6; round++) {
  const server = Deno.serve(
    { hostname: "127.0.0.1", port: 4567, onListen() {} },
    () => new Response("ok"),
  );
  const resp = await fetch("http://127.0.0.1:4567");
  const text = await resp.text();
  if (text !== "ok") {
    throw new Error(`Round ${round}: expected "ok", got "${text}"`);
  }
  await server.shutdown();
  console.log(`Round ${round}: ok`);
}
