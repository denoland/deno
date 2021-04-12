// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

const addr = Deno.args[0] || "127.0.0.1:4500";
const [hostname, port] = addr.split(":");
const listener = Deno.listen({ hostname, port: Number(port) });
console.log("Server listening on", addr);

const body = Deno.core.encode("Hello World");
const response = new Response(body);

for await (const conn of listener) {
  (async () => {
    const requests = Deno.serveHttp(conn);
    for await (const { respondWith } of requests) {
      respondWith(response);
    }
  })();
}
