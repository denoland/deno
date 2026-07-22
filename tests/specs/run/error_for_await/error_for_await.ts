const listener = Deno.listen({ port: 8080 });

for await (const conn of listener) {
  handleConn(conn);
}

function handleConn(conn: Deno.Conn) {
  const httpConn = (Deno as any).serveHttp(conn);
  for await (const event of httpConn) {
    event.respondWith(new Response("html", { status: 200 }));
  }
}
