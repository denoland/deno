// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import { serve, ServerRequest } from "../std/http/server.ts";

const addr = Deno.args[0] || "127.0.0.1:4500";
const originAddr = Deno.args[1] || "127.0.0.1:4501";
const server = serve(addr);

async function proxyRequest(req: ServerRequest): Promise<void> {
  const url = `http://${originAddr}${req.url}`;
  const resp = await fetch(url, {
    method: req.method,
    headers: req.headers,
  });
  req.respond(resp);
}

console.log(`Proxy listening on http://${addr}/`);
for await (const req of server) {
  proxyRequest(req);
}
