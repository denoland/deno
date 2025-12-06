// Copyright 2018-2025 the Deno authors. MIT license.
Deno.serve({
  port: 4506,
  onListen() {
    postMessage("ready");
  },
  handler() {
    return new Response("Hello world");
  },
});
