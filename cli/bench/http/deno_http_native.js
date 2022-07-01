// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

const addr = Deno.args[0] || "127.0.0.1:4500";
const [hostname, port] = addr.split(":");
const listener = Deno.listen({ hostname, port: Number(port) });
console.log("Server listening on", addr);

const encoder = new TextEncoder();
const body = encoder.encode("Hello World");

async function handle(conn) {
  const requests = Deno.serveHttp(conn);
  for await (const event of requests) {
    event.respondWith(new Response(body));
  }
}

for await (const conn of listener) {
  handle(conn);
}
