// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
const addr = Deno.args[1] || "localhost:4555";

function proxyServer() {
  const [hostname, p] = addr.split(":");
  const port = parseInt(p ?? 4555);
  Deno.serve({ hostname, port }, handler);
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
  return new Response(await resp.bytes(), {
    status: resp.status,
    headers: resp.headers,
  });
}

function assertSuccessOutput(output: Deno.CommandOutput) {
  if (output.code !== 0) {
    console.error("STDOUT", new TextDecoder().decode(output.stdout));
    console.error("STDERR", new TextDecoder().decode(output.stderr));
    throw new Error(`Expected exit code 0, was ${output.code}`);
  }
}

async function testFetch() {
  const output = await new Deno.Command(Deno.execPath(), {
    args: [
      "run",
      "--quiet",
      "--reload",
      "--allow-net",
      "proxy_client.ts",
    ],
    env: {
      HTTP_PROXY: `http://${addr}`,
    },
  }).output();

  assertSuccessOutput(output);
}

async function testModuleDownload() {
  const output = await new Deno.Command(Deno.execPath(), {
    args: [
      "cache",
      "--reload",
      "--quiet",
      "http://localhost:4545/run/045_mod.ts",
    ],
    env: {
      HTTP_PROXY: `http://${addr}`,
    },
  }).output();

  assertSuccessOutput(output);
}

async function testFetchNoProxy() {
  const output = await new Deno.Command(Deno.execPath(), {
    args: [
      "run",
      "--quiet",
      "--reload",
      "--allow-net",
      "proxy_client.ts",
    ],
    env: {
      HTTP_PROXY: "http://not.exising.proxy.server",
      NO_PROXY: "localhost",
    },
  }).output();

  assertSuccessOutput(output);
}

async function testModuleDownloadNoProxy() {
  const output = await new Deno.Command(Deno.execPath(), {
    args: [
      "cache",
      "--reload",
      "--quiet",
      "http://localhost:4545/run/045_mod.ts",
    ],
    env: {
      HTTP_PROXY: "http://not.exising.proxy.server",
      NO_PROXY: "localhost",
    },
  }).output();

  assertSuccessOutput(output);
}

async function testFetchProgrammaticProxy() {
  const output = await new Deno.Command(Deno.execPath(), {
    args: [
      "run",
      "--quiet",
      "--reload",
      "--allow-net=localhost:4545,localhost:4555",
      "programmatic_proxy_client.ts",
    ],
  }).output();

  assertSuccessOutput(output);
}

proxyServer();
await testFetch();
await testModuleDownload();
await testFetchNoProxy();
await testModuleDownloadNoProxy();
await testFetchProgrammaticProxy();
Deno.exit(0);
