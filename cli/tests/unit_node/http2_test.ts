// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import * as http2 from "node:http2";
import * as net from "node:net";
import { deferred } from "../../../test_util/std/async/deferred.ts";
import { assertEquals } from "../../../test_util/std/testing/asserts.ts";

// TODO(bartlomieju): reenable sanitizers
Deno.test("[node/http2 client]", {
  sanitizeOps: false,
  sanitizeResources: false,
}, async () => {
  // Create a server to respond to the HTTP2 requests
  const client = http2.connect("http://localhost:4246", {});
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

  const endPromise = deferred();
  setTimeout(() => {
    try {
      client.close();
    } catch (_) {
      // pass
    }
    endPromise.resolve();
  }, 2000);

  await endPromise;
  assertEquals(receivedHeaders, { ":status": 200 });
  assertEquals(receivedData, "hello world\n");

  // TODO(bartlomieju): this is currently not working properly
  assertEquals(receivedTrailers, undefined);
  // Should be this:
  // assertEquals(receivedTrailers, {
  //   "abc": "def",
  //   "opr": "stv",
  //   "foo": "bar",
  // });
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
