// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { serve } from "./server.ts";
import { randomPort } from "./test_util.ts";

const addr = Deno.args[0] || "127.0.0.1:" + randomPort();
const server = serve(addr);
const body = new TextEncoder().encode("Hello World");

console.log(`http://${addr}/`);
for await (const req of server) {
  const res = {
    body,
    headers: new Headers()
  };
  res.headers.set("Date", new Date().toUTCString());
  res.headers.set("Connection", "keep-alive");
  req.respond(res).catch(() => {});
}
