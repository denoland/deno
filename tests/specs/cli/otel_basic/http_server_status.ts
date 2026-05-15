const server = Deno.serve({
  port: 0,
  async onListen({ port }) {
    try {
      await (await fetch(`http://localhost:${port}/ok`)).text();
      await (await fetch(`http://localhost:${port}/not-found`)).text();
      await (await fetch(`http://localhost:${port}/error`)).text();
    } finally {
      server.shutdown();
    }
  },
  handler: (req) => {
    const url = new URL(req.url);
    if (url.pathname === "/ok") {
      return new Response("ok", { status: 200 });
    } else if (url.pathname === "/not-found") {
      return new Response("not found", { status: 404 });
    } else {
      return new Response("internal server error", { status: 500 });
    }
  },
});
