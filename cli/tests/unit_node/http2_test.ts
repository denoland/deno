// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import * as http2 from "node:http2";
import * as net from "node:net";
import { deferred } from "../../../test_util/std/async/deferred.ts";

Deno.test("[node/http2 client]", { ignore: true }, async () => {
  // Create a server to respond to the HTTP2 requests
  const portPromise = deferred();
  const reqPromise = deferred<Request>();
  const ready = deferred();
  const ac = new AbortController();
  const server = Deno.serve({
    port: 8443,
    signal: ac.signal,
    onListen: () => portPromise.resolve(),
    handler: async (req: Request) => {
      reqPromise.resolve(req);
      await ready;
      return new Response("Hello world", {
        headers: { "resp-header-name": "resp-header-value" },
      });
    },
  });

  await portPromise;

  const client = http2.connect("http://localhost:8443", {});
  client.on("error", (err) => console.error(err));

  const req = client.request({ ":method": "POST", ":path": "/" }, {
    waitForTrailers: true,
  });
  console.log("asdf");
  req.on("response", (headers, _flags) => {
    // deno-lint-ignore guard-for-in
    for (const name in headers) {
      console.log(`${name}: ${headers[name]}`);
    }
  });

  req.write("hello");
  console.log("asdf2");
  req.setEncoding("utf8");
  req.on("wantTrailers", () => {
    req.sendTrailers({ foo: "bar" });
  });
  let data = "";
  req.on("data", (chunk) => {
    data += chunk;
  });
  req.on("end", () => {
  });
  req.end();

  const endPromise = deferred();
  setTimeout(() => {
    try {
      client.close();
    } catch (_) {
      // pass
    }
    endPromise.resolve();
  }, 2000);

  // TODO(bartlomieju): not working correctly
  // assertEquals(data, "Hello world");

  await endPromise;
  ac.abort();
  await server.finished;
});

Deno.test("[node/http2 server]", async () => {
  const server = http2.createServer();
  server.listen(0);
  const port = (<net.AddressInfo> server.address()).port;
  const sessionPromise = new Promise<http2.Http2Session>((resolve) =>
    server.on("session", resolve)
  );

  const responsePromise = fetch(`http://localhost:${port}/path`, {
    method: "POST",
    body: "body",
  });

  const session = await sessionPromise;
  const stream = await new Promise<http2.ServerHttp2Stream>((resolve) =>
    session.on("stream", resolve)
  );
  const _headers = await new Promise((resolve) =>
    stream.on("headers", resolve)
  );
  const _data = await new Promise((resolve) => stream.on("data", resolve));
  const _end = await new Promise((resolve) => stream.on("end", resolve));
  stream.respond();
  stream.end();
  const resp = await responsePromise;
  await resp.text();

  await new Promise((resolve) => server.close(resolve));
});
