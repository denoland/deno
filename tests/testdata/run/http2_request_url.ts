Deno.serve({
  port: Number(Deno.args[0]),
  handler(request) {
    const href = new URL(request.url).href;
    return new Response(href);
  },
  onListen() {
    console.log("READY");
  },
});
