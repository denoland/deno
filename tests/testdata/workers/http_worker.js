// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

// deno-lint-ignore-file no-deprecated-deno-api

const listener = Deno.listen({ hostname: "127.0.0.1", port: 4506 });
postMessage("ready");
for await (const conn of listener) {
  (async () => {
    // @ts-ignore `Deno.serveHttp()` was soft-removed in Deno 2.
    const requests = Deno.serveHttp(conn);
    for await (const { respondWith } of requests) {
      respondWith(new Response("Hello world"));
    }
  })();
}
