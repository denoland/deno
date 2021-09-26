const listener = Deno.listen({
  port: Number(Deno.args[0]),
});

console.log("READY");

for await (const conn of listener) {
  for await (const { request, respondWith } of Deno.serveHttp(conn)) {
    const href = new URL(request.url).href;
    respondWith(new Response(href));
  }
}
