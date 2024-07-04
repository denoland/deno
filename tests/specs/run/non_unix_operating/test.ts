// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

const ac = new AbortController();

const server = Deno.serve({
  path: "path/to/socket",
  handler: (_req) => new Response("Hello, world"),
  signal: ac.signal,
  onListen({ _path }) {
    console.log("Server started at ${path}");
  },
});
server.finished.then(() => console.log("Server closed"));

console.log("Closing server...");
ac.abort();
