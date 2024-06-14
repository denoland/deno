// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

import * as http2 from "node:http2";
import { Buffer } from "node:buffer";
import { readFile } from "node:fs/promises";
import { join } from "node:path";
import * as net from "node:net";
import { assert, assertEquals } from "@std/assert/mod.ts";
import { curlRequest } from "../unit/test_util.ts";

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

Deno.test("[node/http2 client GET https://www.example.com]", async () => {
  const clientSession = http2.connect("https://www.example.com");
  const req = clientSession.request({
    ":method": "GET",
    ":path": "/",
  });
  let headers = {};
  let status: number | undefined = 0;
  let chunk = new Uint8Array();
  const endPromise = Promise.withResolvers<void>();
  req.on("response", (h) => {
    status = h[":status"];
    headers = h;
  });
  req.on("data", (c) => {
    chunk = c;
  });
  req.on("end", () => {
    clientSession.close();
    req.close();
    endPromise.resolve();
  });
  req.end();
  await endPromise.promise;
  assert(Object.keys(headers).length > 0);
  assertEquals(status, 200);
  assert(chunk.length > 0);
});

Deno.test("[node/http2.createServer()]", {
  // TODO(satyarohith): enable the test on windows.
  ignore: Deno.build.os === "windows",
}, async () => {
  const server = http2.createServer((_req, res) => {
    res.setHeader("Content-Type", "text/html");
    res.setHeader("X-Foo", "bar");
    res.writeHead(200, { "Content-Type": "text/plain; charset=utf-8" });
    res.write("Hello, World!");
    res.end();
  });
  server.listen(0);
  const port = (<net.AddressInfo> server.address()).port;
  const endpoint = `http://localhost:${port}`;

  const response = await curlRequest([
    endpoint,
    "--http2-prior-knowledge",
  ]);
  assertEquals(response, "Hello, World!");
  server.close();
  // Wait to avoid leaking the timer from here
  // https://github.com/denoland/deno/blob/749b6e45e58ac87188027f79fe403d130f86bd73/ext/node/polyfills/net.ts#L2396-L2402
  // Issue: https://github.com/denoland/deno/issues/22764
  await new Promise<void>((resolve) => server.on("close", resolve));
});

Deno.test("[node/http2 client] write image buffer on request stream works", async () => {
  const url = "https://localhost:5545";
  const client = http2.connect(url);
  client.on("error", (err) => console.error(err));

  const imagePath = join(import.meta.dirname!, "testdata", "green.jpg");
  const buffer = await readFile(imagePath);
  const req = client.request({ ":method": "POST", ":path": "/echo_server" });
  req.write(buffer, (err) => {
    if (err) throw err;
  });

  let receivedData: Buffer;
  req.on("data", (chunk) => {
    if (!receivedData) {
      receivedData = chunk;
    } else {
      receivedData = Buffer.concat([receivedData, chunk]);
    }
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

  await endPromise.promise;
  assertEquals(receivedData!, buffer);
});
