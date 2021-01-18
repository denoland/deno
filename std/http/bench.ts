// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import { serve } from "./server.ts";

const addr = Deno.args[0] || "127.0.0.1:4500";
const server = serve(addr);
const body = new TextEncoder().encode("Hello World");

console.log(`http://${addr}/`);
for await (const req of server) {
  const res = {
    body,
    headers: new Headers(),
  };
  res.headers.set("Date", new Date().toUTCString());
  res.headers.set("Connection", "keep-alive");
  req.respond(res).catch(() => {});
}
