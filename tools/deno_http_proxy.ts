// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import {
  serve,
  ServerRequest
} from "../js/deps/https/deno.land/std/http/server.ts";

const addr = Deno.args[1] || "127.0.0.1:4500";
const originAddr = Deno.args[2] || "127.0.0.1:4501";
const server = serve(addr);

async function main(): Promise<void> {
  console.log(`Proxy listening on http://${addr}/`);
  for await (const req of server) {
    proxyRequest(req);
  }
}

async function proxyRequest(req: ServerRequest) {
  const url = `http://${originAddr}${req.url}`;
  const resp = await fetch(url, {
    method: req.method,
    headers: req.headers
  });
  req.respond(resp);
}

main();
