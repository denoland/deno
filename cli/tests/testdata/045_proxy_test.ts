// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import { serve, ServerRequest } from "../../../test_util/std/http/server.ts";
import { assertEquals } from "../../../test_util/std/testing/asserts.ts";

const addr = Deno.args[1] || "127.0.0.1:4555";

async function proxyServer() {
  const server = serve(addr);

  console.log(`Proxy server listening on http://${addr}/`);
  for await (const req of server) {
    proxyRequest(req);
  }
}

async function proxyRequest(req: ServerRequest) {
  console.log(`Proxy request to: ${req.url}`);
  const proxyAuthorization = req.headers.get("proxy-authorization");
  if (proxyAuthorization) {
    console.log(`proxy-authorization: ${proxyAuthorization}`);
    req.headers.delete("proxy-authorization");
  }
  const resp = await fetch(req.url, {
    method: req.method,
    headers: req.headers,
  });
  req.respond({
    status: resp.status,
    body: new Uint8Array(await resp.arrayBuffer()),
    headers: resp.headers,
  });
}

async function testFetch() {
  const c = Deno.run({
    cmd: [
      Deno.execPath(),
      "run",
      "--quiet",
      "--reload",
      "--allow-net",
      "045_proxy_client.ts",
    ],
    stdout: "piped",
    env: {
      HTTP_PROXY: `http://${addr}`,
    },
  });

  const status = await c.status();
  assertEquals(status.code, 0);
  c.close();
}

async function testModuleDownload() {
  const http = Deno.run({
    cmd: [
      Deno.execPath(),
      "cache",
      "--reload",
      "--quiet",
      "http://localhost:4545/045_mod.ts",
    ],
    stdout: "piped",
    env: {
      HTTP_PROXY: `http://${addr}`,
    },
  });

  const httpStatus = await http.status();
  assertEquals(httpStatus.code, 0);
  http.close();
}

async function testFetchNoProxy() {
  const c = Deno.run({
    cmd: [
      Deno.execPath(),
      "run",
      "--quiet",
      "--reload",
      "--allow-net",
      "045_proxy_client.ts",
    ],
    stdout: "piped",
    env: {
      HTTP_PROXY: "http://not.exising.proxy.server",
      NO_PROXY: "localhost",
    },
  });

  const status = await c.status();
  assertEquals(status.code, 0);
  c.close();
}

async function testModuleDownloadNoProxy() {
  const http = Deno.run({
    cmd: [
      Deno.execPath(),
      "cache",
      "--reload",
      "--quiet",
      "http://localhost:4545/045_mod.ts",
    ],
    stdout: "piped",
    env: {
      HTTP_PROXY: "http://not.exising.proxy.server",
      NO_PROXY: "localhost",
    },
  });

  const httpStatus = await http.status();
  assertEquals(httpStatus.code, 0);
  http.close();
}

async function testFetchProgrammaticProxy() {
  const c = Deno.run({
    cmd: [
      Deno.execPath(),
      "run",
      "--quiet",
      "--reload",
      "--allow-net=localhost:4545,localhost:4555",
      "--unstable",
      "045_programmatic_proxy_client.ts",
    ],
    stdout: "piped",
  });
  const status = await c.status();
  assertEquals(status.code, 0);
  c.close();
}

proxyServer();
await testFetch();
await testModuleDownload();
await testFetchNoProxy();
await testModuleDownloadNoProxy();
await testFetchProgrammaticProxy();
Deno.exit(0);
