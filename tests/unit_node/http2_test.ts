// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

// deno-lint-ignore-file no-console

import * as http2 from "node:http2";
import { Buffer } from "node:buffer";
import { readFile } from "node:fs/promises";
import { join } from "node:path";
import * as net from "node:net";
import { assert, assertEquals } from "@std/assert";
import { curlRequest } from "../unit/test_util.ts";

for (const url of ["http://localhost:4246", "https://localhost:4247"]) {
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
  const port = (server.address() as net.AddressInfo).port;
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

Deno.test("[node/http2 client] write 512kb buffer on request stream works", async () => {
  const url = "https://localhost:5545";
  const client = http2.connect(url);
  client.on("error", (err) => console.error(err));

  const filePath = join(
    import.meta.dirname!,
    "testdata",
    "lorem_ipsum_512kb.txt",
  );
  const buffer = await readFile(filePath);
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

// https://github.com/denoland/deno/issues/24678
Deno.test("[node/http2 client] deno doesn't panic on uppercase headers", async () => {
  const url = "http://127.0.0.1:4246";
  const client = http2.connect(url);
  client.on("error", (err) => console.error(err));

  // The "User-Agent" header has uppercase characters to test the panic.
  const req = client.request({
    ":method": "POST",
    ":path": "/",
    "User-Agent": "http2",
  });
  const endPromise = Promise.withResolvers<void>();

  let receivedData = "";

  req.write("hello");
  req.setEncoding("utf8");

  req.on("data", (chunk) => {
    receivedData += chunk;
  });
  req.on("end", () => {
    req.close();
    client.close();
    endPromise.resolve();
  });
  req.end();
  await endPromise.promise;
  assertEquals(receivedData, "hello world\n");
});

Deno.test("[node/http2 ClientHttp2Session.socket]", async () => {
  const url = "http://127.0.0.1:4246";
  const client = http2.connect(url);
  client.on("error", (err) => console.error(err));

  const req = client.request({ ":method": "POST", ":path": "/" });
  const endPromise = Promise.withResolvers<void>();

  // test that we can access session.socket
  client.socket.setTimeout(10000);
  // nodejs allows setting arbitrary properties
  // deno-lint-ignore no-explicit-any
  (client.socket as any).nonExistant = 9001;
  // deno-lint-ignore no-explicit-any
  assertEquals((client.socket as any).nonExistant, 9001);

  // regular request dance to make sure it keeps working
  let receivedData = "";
  req.write("hello");
  req.setEncoding("utf8");

  req.on("data", (chunk) => {
    receivedData += chunk;
  });
  req.on("end", () => {
    req.close();
    client.close();
    endPromise.resolve();
  });
  req.end();
  await endPromise.promise;
  assertEquals(client.socket.remoteAddress, "127.0.0.1");
  assertEquals(client.socket.remotePort, 4246);
  assertEquals(client.socket.remoteFamily, "IPv4");
  client.socket.setTimeout(0);
  assertEquals(receivedData, "hello world\n");
});

Deno.test("[node/http2 client] connection states", async () => {
  const expected = {
    beforeConnect: { connecting: true, closed: false, destroyed: false },
    afterConnect: { connecting: false, closed: false, destroyed: false },
    afterClose: { connecting: false, closed: true, destroyed: false },
    afterDestroy: { connecting: false, closed: true, destroyed: true },
  };
  const actual: Partial<typeof expected> = {};

  const url = "http://127.0.0.1:4246";
  const connectPromise = Promise.withResolvers<void>();
  const client = http2.connect(url, {}, () => {
    connectPromise.resolve();
  });
  client.on("error", (err) => console.error(err));

  // close event happens after destory has been called
  const destroyPromise = Promise.withResolvers<void>();
  client.on("close", () => {
    destroyPromise.resolve();
  });

  actual.beforeConnect = {
    connecting: client.connecting,
    closed: client.closed,
    destroyed: client.destroyed,
  };

  await connectPromise.promise;
  actual.afterConnect = {
    connecting: client.connecting,
    closed: client.closed,
    destroyed: client.destroyed,
  };

  // leave a request open to prevent immediate destroy
  const req = client.request();
  req.on("data", () => {});
  req.on("error", (err) => console.error(err));
  const reqClosePromise = Promise.withResolvers<void>();
  req.on("close", () => {
    reqClosePromise.resolve();
  });

  client.close();
  actual.afterClose = {
    connecting: client.connecting,
    closed: client.closed,
    destroyed: client.destroyed,
  };

  await destroyPromise.promise;
  actual.afterDestroy = {
    connecting: client.connecting,
    closed: client.closed,
    destroyed: client.destroyed,
  };

  await reqClosePromise.promise;

  assertEquals(actual, expected);
});

Deno.test("request and response exports", () => {
  assert(http2.Http2ServerRequest);
  assert(http2.Http2ServerResponse);
});
