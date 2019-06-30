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
  // TODO(kt3k): lib.deno_runtime.d.ts has 2 EOF types: Deno.EOF and io.EOF.
  // They are identical symbols, and should be compatible. However typescript
  // recognizes they are different types and the below call doesn't compile.
  // @ts-ignore
  req.respond(resp);
}

main();
