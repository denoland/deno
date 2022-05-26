// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
import { Server } from "../../../test_util/std/http/server.ts";
import { assertEquals } from "../../../test_util/std/testing/asserts.ts";

const addr = Deno.args[1] || "localhost:4555";

async function proxyServer() {
  const [hostname, p] = addr.split(":");
  const port = parseInt(p ?? 4555);
  const server = new Server({ hostname, port, handler });
  console.log(`Proxy server listening on http://${addr}/`);
  await server.listenAndServe();
}

async function handler(req: Request): Promise<Response> {
  console.log(`Proxy request to: ${req.url}`);
  const headers = new Headers(req.headers);
  const proxyAuthorization = headers.get("proxy-authorization");
  if (proxyAuthorization) {
    console.log(`proxy-authorization: ${proxyAuthorization}`);
    headers.delete("proxy-authorization");
  }
  const resp = await fetch(req.url, {
    method: req.method,
    headers: headers,
  });
  return new Response(new Uint8Array(await resp.arrayBuffer()), {
    status: resp.status,
    headers: resp.headers,
  });
}

async function testFetch() {
  const { status } = await Deno.spawn(Deno.execPath(), {
    args: [
      "run",
      "--quiet",
      "--reload",
      "--allow-net",
      "045_proxy_client.ts",
    ],
    env: {
      HTTP_PROXY: `http://${addr}`,
    },
  });

  assertEquals(status.code, 0);
}

async function testModuleDownload() {
  const { status } = await Deno.spawn(Deno.execPath(), {
    args: [
      "cache",
      "--reload",
      "--quiet",
      "http://localhost:4545/045_mod.ts",
    ],
    env: {
      HTTP_PROXY: `http://${addr}`,
    },
  });

  assertEquals(status.code, 0);
}

async function testFetchNoProxy() {
  const { status } = await Deno.spawn(Deno.execPath(), {
    args: [
      "run",
      "--quiet",
      "--reload",
      "--allow-net",
      "045_proxy_client.ts",
    ],
    env: {
      HTTP_PROXY: "http://not.exising.proxy.server",
      NO_PROXY: "localhost",
    },
  });

  assertEquals(status.code, 0);
}

async function testModuleDownloadNoProxy() {
  const { status } = await Deno.spawn(Deno.execPath(), {
    args: [
      "cache",
      "--reload",
      "--quiet",
      "http://localhost:4545/045_mod.ts",
    ],
    env: {
      HTTP_PROXY: "http://not.exising.proxy.server",
      NO_PROXY: "localhost",
    },
  });

  assertEquals(status.code, 0);
}

async function testFetchProgrammaticProxy() {
  const { status } = await Deno.spawn(Deno.execPath(), {
    args: [
      "run",
      "--quiet",
      "--reload",
      "--allow-net=localhost:4545,localhost:4555",
      "--unstable",
      "045_programmatic_proxy_client.ts",
    ],
  });
  assertEquals(status.code, 0);
}

proxyServer();
await testFetch();
await testModuleDownload();
await testFetchNoProxy();
await testModuleDownloadNoProxy();
await testFetchProgrammaticProxy();
Deno.exit(0);
