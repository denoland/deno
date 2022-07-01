// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

const addr = Deno.args[0] || "127.0.0.1:4500";
const [hostname, port] = addr.split(":");
const listener = Deno.listen({ hostname, port: Number(port) });
console.log("Server listening on", addr);

for await (const conn of listener) {
  (async () => {
    const requests = Deno.serveHttp(conn);
    for await (const { respondWith, request } of requests) {
      if (request.method == "POST") {
        const json = await request.json();
        respondWith(new Response(json.hello))
          .catch((e) => console.log(e));
      }
    }
  })();
}
