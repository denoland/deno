Deno.serve({ port: 8001 }, (req) => {
  return new Response("Hello world");
});

const proxyUrl = "http://localhost:8001/";

async function handleHttp(conn: Deno.Conn) {
  for await (const e of Deno.serveHttp(conn)) {
    e.respondWith(serve(e.request));
  }
}

let listener = Deno.listen({ port: 8000 });
let id = setTimeout(() => Deno.exit(0), 1000);

(async () => {
  for await (const conn of listener) {
    handleHttp(conn);
  }
})();

(async () => {
  let conn = await Deno.connect({ host: "localhost", port: 8000 });
  const payload = new TextEncoder().encode(
    "POST /api/sessions HTTP/1.1\x0d\x0aConnection: keep-alive\x0d\x0aContent-Length: 2\x0d\x0a\x0d\x0a{}",
  );
  await conn.write(payload);
  // conn.close()
})();

async function serve(req) {
  console.log("proxying");
  const r = await fetch("http://localhost:8001/", req);
  console.log("proxied");
  return r;
}
