// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import * as http2 from "node:http2";
import * as net from "node:net";
import { assertEquals } from "../../../test_util/std/assert/mod.ts";

for (const url of ["http://127.0.0.1:4246", "https://127.0.0.1:4247"]) {
  Deno.test(`[node/http2 client] ${url}`, {
    ignore: Deno.build.os === "windows",
  }, async () => {
    // Create a server to respond to the HTTP2 requests
    const client = http2.connect(url, {});
    client.on("error", (err) => console.error(err));

    const req = client.request({ ":method": "POST", ":path": "/" }, {
      waitForTrailers: true,
    });

    let receivedTrailers;
    let receivedHeaders;
    let receivedData = "";

    req.on("response", (headers, _flags) => {
      receivedHeaders = headers;
    });

    req.write("hello");
    req.setEncoding("utf8");

    req.on("wantTrailers", () => {
      req.sendTrailers({ foo: "bar" });
    });

    req.on("trailers", (trailers, _flags) => {
      receivedTrailers = trailers;
    });

    req.on("data", (chunk) => {
      receivedData += chunk;
    });
    req.end();

    const { promise, resolve } = Promise.withResolvers<void>();
    setTimeout(() => {
      try {
        client.close();
      } catch (_) {
        // pass
      }
      resolve();
    }, 2000);

    await promise;
    assertEquals(receivedHeaders, { ":status": 200 });
    assertEquals(receivedData, "hello world\n");

    assertEquals(receivedTrailers, {
      "abc": "def",
      "opr": "stv",
      "foo": "bar",
    });
  });
}

Deno.test(`[node/http2 client createConnection]`, {
  ignore: Deno.build.os === "windows",
}, async () => {
  const url = "http://127.0.0.1:4246";
  const createConnDeferred = Promise.withResolvers<void>();
  // Create a server to respond to the HTTP2 requests
  const client = http2.connect(url, {
    createConnection() {
      const socket = net.connect({ host: "127.0.0.1", port: 4246 });

      socket.on("connect", () => {
        createConnDeferred.resolve();
      });

      return socket;
    },
  });
  client.on("error", (err) => console.error(err));

  const req = client.request({ ":method": "POST", ":path": "/" });

  let receivedData = "";

  req.write("hello");
  req.setEncoding("utf8");

  req.on("data", (chunk) => {
    receivedData += chunk;
  });
  req.end();

  const endPromise = Promise.withResolvers<void>();
  setTimeout(() => {
    try {
      client.close();
    } catch (_) {
      // pass
    }
    endPromise.resolve();
  }, 2000);

  await createConnDeferred.promise;
  await endPromise.promise;
  assertEquals(receivedData, "hello world\n");
});

// TODO(bartlomieju): reenable sanitizers
Deno.test("[node/http2 server]", { sanitizeOps: false }, async () => {
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
  await new Promise((resolve) => stream.on("headers", resolve));
  await new Promise((resolve) => stream.on("data", resolve));
  await new Promise((resolve) => stream.on("end", resolve));
  stream.respond();
  stream.end();
  const resp = await responsePromise;
  await resp.text();

  await new Promise((resolve) => server.close(resolve));
});
