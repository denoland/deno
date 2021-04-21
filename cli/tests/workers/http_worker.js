// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
const listener = Deno.listen({ hostname: "127.0.0.1", port: 4500 });
for await (const conn of listener) {
  (async () => {
    const requests = Deno.serveHttp(conn);
    for await (const { respondWith } of requests) {
      respondWith(new Response("Hello world"));
    }
  })();
}
