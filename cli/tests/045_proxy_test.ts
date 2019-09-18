// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import {
  serve,
  ServerRequest
} from "../../js/deps/https/deno.land/std/http/server.ts";

const addr = Deno.args[1] || "127.0.0.1:4500";

async function proxyServer() {
  const server = serve(addr);

  console.log(`Proxy listening on http://${addr}/`);
  for await (const req of server) {
    proxyRequest(req);
  }
}

async function proxyRequest(req: ServerRequest): Promise<void> {
  console.log(`Proxying request to: ${req.url}`);
  const resp = await fetch(req.url, {
    method: req.method,
    headers: req.headers
  });
  req.respond(resp);
}

async function main(): Promise<void> {
  proxyServer();

  const c = Deno.run({
    args: [
      Deno.execPath(),
      "--no-prompt",
      "--allow-net",
      "cli/tests/045_proxy_client.ts"
    ],
    stdout: "piped",
    env: {
      HTTP_PROXY: `http://${addr}`,
      HTTPS_PROXY: `http://${addr}`
    }
  });

  console.log("before status");
  await c.status();
  console.log("AFTER status");
  const clientOutput = await c.output();
  console.log("Client output", clientOutput);
  c.close();
}

main();
