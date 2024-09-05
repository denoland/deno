const listener = Deno.listen({ port: 8080 });

for await (const conn of listener) {
  handleConn(conn);
}

function handleConn(conn: Deno.Conn) {
  // @ts-ignore `Deno.serveHttp()` was soft-removed in Deno 2.
  const httpConn = Deno.serveHttp(conn);
  for await (const event of httpConn) {
    event.respondWith(new Response("html", { status: 200 }));
  }
}
