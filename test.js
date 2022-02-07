async function server() {
  let tcp_server = Deno.listen({ host: "127.0.0.1", port: 8080 });
  let tcp_conn = await tcp_server.accept();
  let http_conn = Deno.serveHttp(tcp_conn);
  let http_event = await http_conn.nextRequest();
  console.log("waiting for upgrade");
  http_event.respondWith(new Response(null));
  const r = await http_event.upgrade();
  console.log(r.connRid, r.connType, new TextDecoder().decode(r.readBuf));
}

async function client() {
  let tcp_conn = await Deno.connect({ host: "127.0.0.1", port: 8080 });
  tcp_conn.write(
    new TextEncoder().encode(
      "CONNECT server.example.com:80 HTTP/1.1\r\n\r\nBla bla bla\nbla bla\nbla\n",
    ),
  );
}

await Promise.all([server(), client()]);
