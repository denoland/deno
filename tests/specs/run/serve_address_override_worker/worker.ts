Deno.serve({
  port: 9005, // must NOT be overridden: main already consumed the override
  onListen({ port }) {
    self.postMessage(port);
  },
}, () => new Response("worker"));
