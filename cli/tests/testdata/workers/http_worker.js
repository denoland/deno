// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
const listener = Deno.listen({ hostname: "127.0.0.1", port: 4506 });
postMessage("ready");
for await (const conn of listener) {
  (async () => {
    const requests = Deno.serveHttp(conn);
    for await (const { respondWith } of requests) {
      respondWith(new Response("Hello world"));
    }
  })();
}
