const addr = Deno.args[0] || "127.0.0.1:4500";
const [hostname, port] = addr.split(":");
const listener = Deno.listen({ hostname, port: Number(port) });
console.log("Server listening on", addr);

for await (const conn of listener) {
  (async () => {
    const requests = Deno.serveHttp(conn);
    for await (const { respondWith } of requests) {
      respondWith(
        new Response("Hello World", {
          status: 200,
          headers: {
            server: "deno",
            "content-type": "text/plain",
          },
        }),
      )
        .catch((e) => console.log(e));
    }
  })();
}
