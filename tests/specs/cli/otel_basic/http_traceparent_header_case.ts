// The HTTP/1 fast path hands header names to the propagator verbatim from the
// wire, so send a raw request with a non-lowercase `Traceparent` header name.
// `fetch` cannot be used here because it lowercases header names itself.
const server = Deno.serve({
  port: 0,
  async onListen({ port }) {
    try {
      const conn = await Deno.connect({ hostname: "127.0.0.1", port });
      await conn.write(
        new TextEncoder().encode(
          "GET / HTTP/1.1\r\n" +
            "Host: localhost\r\n" +
            "Traceparent: 00-0af7651916cd43dd8448eb211c80319c-b7ad6b7169203331-01\r\n" +
            "Connection: close\r\n" +
            "\r\n",
        ),
      );
      await new Response(conn.readable).text();
    } finally {
      server.shutdown();
    }
  },
  handler: () => new Response("ok"),
});
