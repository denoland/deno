// Copyright 2018-2026 the Deno authors. MIT license.

// Regression test for https://github.com/denoland/deno/issues/25308
// kv.listenQueue(...) (called without await) must not block subsequent
// async work on the main thread. Before the JS rewrite of ext/kv, the
// dequeue loop could starve sibling tasks (kv.get, Deno.serve) until a
// console.log() was added behind it.

import { kv } from "./kv.ts";

const got = await kv.get(["nope"]);
console.log("get returned:", got.value);

const server = Deno.serve(
  { port: 0, onListen: ({ port }) => console.log("listening", typeof port) },
  () => new Response(),
);
await server.shutdown();
kv.close();
console.log("ok");
