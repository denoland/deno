// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
import { assert, assertEquals, assertThrows, fail } from "./test_util.ts";

const servePort = 4248;
const serveUrl = `ws://localhost:${servePort}/`;

Deno.test({ permissions: "none" }, function websocketPermissionless() {
  assertThrows(
    () => new WebSocket("ws://localhost"),
    Deno.errors.NotCapable,
  );
});

Deno.test(async function websocketConstructorTakeURLObjectAsParameter() {
  const { promise, resolve, reject } = Promise.withResolvers<void>();
  const ws = new WebSocket(new URL("ws://localhost:4242/"));
  assertEquals(ws.url, "ws://localhost:4242/");
  ws.onerror = (e) => reject(e);
  ws.onopen = () => ws.close();
  ws.onclose = () => {
    resolve();
  };
  await promise;
});

Deno.test(async function websocketH2SendSmallPacket() {
  const { promise, resolve, reject } = Promise.withResolvers<void>();
  const ws = new WebSocket(new URL("wss://localhost:4249/"));
  assertEquals(ws.url, "wss://localhost:4249/");
  let messageCount = 0;
  ws.onerror = (e) => reject(e);
  ws.onopen = () => {
    ws.send("a".repeat(16));
    ws.send("a".repeat(16));
    ws.send("a".repeat(16));
  };
  ws.onmessage = () => {
    if (++messageCount == 3) {
      ws.close();
    }
  };
  ws.onclose = () => {
    resolve();
  };
  await promise;
});

Deno.test(async function websocketH2SendLargePacket() {
  const { promise, resolve, reject } = Promise.withResolvers<void>();
  const ws = new WebSocket(new URL("wss://localhost:4249/"));
  assertEquals(ws.url, "wss://localhost:4249/");
  let messageCount = 0;
  ws.onerror = (e) => reject(e);
  ws.onopen = () => {
    ws.send("a".repeat(65000));
    ws.send("a".repeat(65000));
    ws.send("a".repeat(65000));
  };
  ws.onmessage = () => {
    if (++messageCount == 3) {
      ws.close();
    }
  };
  ws.onclose = () => {
    resolve();
  };
  await promise;
});

Deno.test(async function websocketSendLargePacket() {
  const { promise, resolve, reject } = Promise.withResolvers<void>();
  const ws = new WebSocket(new URL("wss://localhost:4243/"));
  assertEquals(ws.url, "wss://localhost:4243/");
  ws.onerror = (e) => reject(e);
  ws.onopen = () => {
    ws.send("a".repeat(65000));
  };
  ws.onmessage = () => {
    ws.close();
  };
  ws.onclose = () => {
    resolve();
  };
  await promise;
});

Deno.test(async function websocketSendLargeBinaryPacket() {
  const { promise, resolve, reject } = Promise.withResolvers<void>();
  const ws = new WebSocket(new URL("wss://localhost:4243/"));
  ws.binaryType = "arraybuffer";
  assertEquals(ws.url, "wss://localhost:4243/");
  ws.onerror = (e) => reject(e);
  ws.onopen = () => {
    ws.send(new Uint8Array(65000));
  };
  ws.onmessage = (msg: MessageEvent) => {
    assertEquals(msg.data.byteLength, 65000);
    ws.close();
  };
  ws.onclose = () => {
    resolve();
  };
  await promise;
});

Deno.test(async function websocketSendLargeBlobPacket() {
  const { promise, resolve, reject } = Promise.withResolvers<void>();
  const ws = new WebSocket(new URL("wss://localhost:4243/"));
  ws.binaryType = "arraybuffer";
  assertEquals(ws.url, "wss://localhost:4243/");
  ws.onerror = (e) => reject(e);
  ws.onopen = () => {
    ws.send(new Blob(["a".repeat(65000)]));
  };
  ws.onmessage = (msg: MessageEvent) => {
    assertEquals(msg.data.byteLength, 65000);
    ws.close();
  };
  ws.onclose = () => {
    resolve();
  };
  await promise;
});

// https://github.com/denoland/deno/pull/17762
// https://github.com/denoland/deno/issues/17761
Deno.test(async function websocketPingPong() {
  const { promise, resolve, reject } = Promise.withResolvers<void>();
  const ws = new WebSocket("ws://localhost:4245/");
  assertEquals(ws.url, "ws://localhost:4245/");
  ws.onerror = (e) => reject(e);
  ws.onmessage = (e) => {
    ws.send(e.data);
  };
  ws.onclose = () => {
    resolve();
  };
  await promise;
  ws.close();
});

// TODO(mmastrac): This requires us to ignore bad certs
// Deno.test(async function websocketSecureConnect() {
//   const { promise, resolve } = Promise.withResolvers<void>();
//   const ws = new WebSocket("wss://localhost:4243/");
//   assertEquals(ws.url, "wss://localhost:4243/");
//   ws.onerror = (error) => {
//     console.log(error);
//     fail();
//   };
//   ws.onopen = () => ws.close();
//   ws.onclose = () => {
//     resolve();
//   };
//   await promise;
// });

// https://github.com/denoland/deno/issues/18700
Deno.test(
  { sanitizeOps: false, sanitizeResources: false },
  async function websocketWriteLock() {
    const ac = new AbortController();
    const listeningDeferred = Promise.withResolvers<void>();

    const server = Deno.serve({
      handler: (req) => {
        const { socket, response } = Deno.upgradeWebSocket(req);
        socket.onopen = function () {
          setTimeout(() => socket.send("Hello"), 500);
        };
        socket.onmessage = function (e) {
          assertEquals(e.data, "Hello");
          ac.abort();
        };
        return response;
      },
      signal: ac.signal,
      onListen: () => listeningDeferred.resolve(),
      hostname: "localhost",
      port: servePort,
    });

    await listeningDeferred.promise;
    const deferred = Promise.withResolvers<void>();
    const ws = new WebSocket(serveUrl);
    assertEquals(ws.url, serveUrl);
    ws.onerror = () => fail();
    ws.onmessage = (e) => {
      assertEquals(e.data, "Hello");
      setTimeout(() => {
        ws.send(e.data);
      }, 1000);
      deferred.resolve();
    };
    ws.onclose = () => {
      deferred.resolve();
    };

    await Promise.all([deferred.promise, server.finished]);
    ws.close();
  },
);

// https://github.com/denoland/deno/issues/18775
Deno.test({
  sanitizeOps: false,
  sanitizeResources: false,
}, async function websocketDoubleClose() {
  const deferred = Promise.withResolvers<void>();

  const ac = new AbortController();
  const listeningDeferred = Promise.withResolvers<void>();

  const server = Deno.serve({
    handler: (req) => {
      const { response, socket } = Deno.upgradeWebSocket(req);
      let called = false;
      socket.onopen = () => socket.send("Hello");
      socket.onmessage = () => {
        assert(!called);
        called = true;
        socket.send("bye");
        socket.close();
      };
      socket.onclose = () => ac.abort();
      socket.onerror = () => fail();
      return response;
    },
    signal: ac.signal,
    onListen: () => listeningDeferred.resolve(),
    hostname: "localhost",
    port: servePort,
  });

  await listeningDeferred.promise;

  const ws = new WebSocket(serveUrl);
  assertEquals(ws.url, serveUrl);
  ws.onerror = () => fail();
  ws.onmessage = (m: MessageEvent) => {
    if (m.data == "Hello") ws.send("bye");
  };
  ws.onclose = () => {
    deferred.resolve();
  };
  await Promise.all([deferred.promise, server.finished]);
});

// https://github.com/denoland/deno/issues/19483
Deno.test({
  sanitizeOps: false,
  sanitizeResources: false,
}, async function websocketCloseFlushes() {
  const deferred = Promise.withResolvers<void>();

  const ac = new AbortController();
  const listeningDeferred = Promise.withResolvers<void>();

  const server = Deno.serve({
    handler: (req) => {
      const { response, socket } = Deno.upgradeWebSocket(req);
      socket.onopen = () => socket.send("Hello");
      socket.onmessage = () => {
        socket.send("Bye");
        socket.close();
      };
      socket.onclose = () => ac.abort();
      socket.onerror = () => fail();
      return response;
    },
    signal: ac.signal,
    onListen: () => listeningDeferred.resolve(),
    hostname: "localhost",
    port: servePort,
  });

  await listeningDeferred.promise;

  const ws = new WebSocket(serveUrl);
  assertEquals(ws.url, serveUrl);
  let seenBye = false;
  ws.onerror = () => fail();
  ws.onmessage = ({ data }) => {
    if (data == "Hello") {
      ws.send("Hello!");
    } else {
      assertEquals(data, "Bye");
      seenBye = true;
    }
  };
  ws.onclose = () => {
    deferred.resolve();
  };
  await Promise.all([deferred.promise, server.finished]);

  assert(seenBye);
});

Deno.test(
  { sanitizeOps: false },
  function websocketConstructorWithPrototypePollution() {
    const originalSymbolIterator = Array.prototype[Symbol.iterator];
    try {
      Array.prototype[Symbol.iterator] = () => {
        throw Error("unreachable");
      };
      assertThrows(() => {
        new WebSocket(
          new URL("ws://localhost:4242/"),
          // Allow `Symbol.iterator` to be called in WebIDL conversion to `sequence<DOMString>`
          // deno-lint-ignore no-explicit-any
          ["soap", "soap"].values() as any,
        );
      }, DOMException);
    } finally {
      Array.prototype[Symbol.iterator] = originalSymbolIterator;
    }
  },
);

Deno.test(async function websocketTlsSocketWorks() {
  const cert = await Deno.readTextFile("tests/testdata/tls/localhost.crt");
  const key = await Deno.readTextFile("tests/testdata/tls/localhost.key");

  const messages: string[] = [],
    errors: { server?: Event; client?: Event }[] = [];
  const promise = new Promise((okay, nope) => {
    const ac = new AbortController();
    const server = Deno.serve({
      handler: (req) => {
        const { response, socket } = Deno.upgradeWebSocket(req);
        socket.onopen = () => socket.send("ping");
        socket.onmessage = (e) => {
          messages.push(e.data);
          socket.close();
        };
        socket.onerror = (e) => errors.push({ server: e });
        socket.onclose = () => ac.abort();
        return response;
      },
      signal: ac.signal,
      hostname: "localhost",
      port: servePort,
      cert,
      key,
    });
    setTimeout(() => {
      const ws = new WebSocket(`wss://localhost:${servePort}`);
      ws.onmessage = (e) => {
        messages.push(e.data);
        ws.send("pong");
      };
      ws.onerror = (e) => {
        errors.push({ client: e });
        nope();
      };
      ws.onclose = () => okay(server.finished);
    }, 1000);
  });

  const finished = await promise;

  assertEquals(errors, []);
  assertEquals(messages, ["ping", "pong"]);

  await finished;
});

// https://github.com/denoland/deno/issues/15340
Deno.test(
  async function websocketServerFieldInit() {
    const ac = new AbortController();
    const listeningDeferred = Promise.withResolvers<void>();

    const server = Deno.serve({
      handler: (req) => {
        const { socket, response } = Deno.upgradeWebSocket(req, {
          idleTimeout: 0,
        });
        socket.onopen = function () {
          assert(typeof socket.url == "string");
          assert(socket.readyState == WebSocket.OPEN);
          assert(socket.protocol == "");
          assert(socket.binaryType == "arraybuffer");
          socket.close();
        };
        socket.onclose = () => ac.abort();
        return response;
      },
      signal: ac.signal,
      onListen: () => listeningDeferred.resolve(),
      hostname: "localhost",
      port: servePort,
    });

    await listeningDeferred.promise;
    const deferred = Promise.withResolvers<void>();
    const ws = new WebSocket(serveUrl);
    assertEquals(ws.url, serveUrl);
    ws.onerror = () => fail();
    ws.onclose = () => {
      deferred.resolve();
    };

    await Promise.all([deferred.promise, server.finished]);
  },
);

Deno.test(
  { sanitizeOps: false },
  async function websocketServerGetsGhosted() {
    const ac = new AbortController();
    const listeningDeferred = Promise.withResolvers<void>();

    const server = Deno.serve({
      handler: (req) => {
        const { socket, response } = Deno.upgradeWebSocket(req, {
          idleTimeout: 2,
        });
        socket.onerror = () => socket.close();
        socket.onclose = () => ac.abort();
        return response;
      },
      signal: ac.signal,
      onListen: () => listeningDeferred.resolve(),
      hostname: "localhost",
      port: servePort,
    });

    await listeningDeferred.promise;
    const r = await fetch("http://localhost:4545/ghost_ws_client");
    assertEquals(r.status, 200);
    await r.body?.cancel();

    await server.finished;
  },
);

Deno.test("invalid scheme", () => {
  assertThrows(() => new WebSocket("foo://localhost:4242"));
});

Deno.test("fragment", () => {
  assertThrows(() => new WebSocket("ws://localhost:4242/#"));
  assertThrows(() => new WebSocket("ws://localhost:4242/#foo"));
});

Deno.test("duplicate protocols", () => {
  assertThrows(() => new WebSocket("ws://localhost:4242", ["foo", "foo"]));
});

Deno.test("invalid server", async () => {
  const { promise, resolve } = Promise.withResolvers<void>();
  const ws = new WebSocket("ws://localhost:2121");
  let err = false;
  ws.onerror = () => {
    err = true;
  };
  ws.onclose = () => {
    if (err) {
      resolve();
    } else {
      fail();
    }
  };
  ws.onopen = () => fail();
  await promise;
});

Deno.test("connect & close", async () => {
  const { promise, resolve } = Promise.withResolvers<void>();
  const ws = new WebSocket("ws://localhost:4242");
  ws.onerror = () => fail();
  ws.onopen = () => {
    ws.close();
  };
  ws.onclose = () => {
    resolve();
  };
  await promise;
});

Deno.test("connect & abort", async () => {
  const { promise, resolve } = Promise.withResolvers<void>();
  const ws = new WebSocket("ws://localhost:4242");
  ws.close();
  let err = false;
  ws.onerror = () => {
    err = true;
  };
  ws.onclose = () => {
    if (err) {
      resolve();
    } else {
      fail();
    }
  };
  ws.onopen = () => fail();
  await promise;
});

Deno.test("connect & close custom valid code", async () => {
  const { promise, resolve } = Promise.withResolvers<void>();
  const ws = new WebSocket("ws://localhost:4242");
  ws.onerror = () => fail();
  ws.onopen = () => ws.close(1000);
  ws.onclose = () => {
    resolve();
  };
  await promise;
});

Deno.test("connect & close custom invalid code", async () => {
  const { promise, resolve } = Promise.withResolvers<void>();
  const ws = new WebSocket("ws://localhost:4242");
  ws.onerror = () => fail();
  ws.onopen = () => {
    assertThrows(() => ws.close(1001));
    ws.close();
  };
  ws.onclose = () => {
    resolve();
  };
  await promise;
});

Deno.test("connect & close custom valid reason", async () => {
  const { promise, resolve } = Promise.withResolvers<void>();
  const ws = new WebSocket("ws://localhost:4242");
  ws.onerror = () => fail();
  ws.onopen = () => ws.close(1000, "foo");
  ws.onclose = () => {
    resolve();
  };
  await promise;
});

Deno.test("connect & close custom invalid reason", async () => {
  const { promise, resolve } = Promise.withResolvers<void>();
  const ws = new WebSocket("ws://localhost:4242");
  ws.onerror = () => fail();
  ws.onopen = () => {
    assertThrows(() => ws.close(1000, "".padEnd(124, "o")));
    ws.close();
  };
  ws.onclose = () => {
    resolve();
  };
  await promise;
});

Deno.test("echo string", async () => {
  const { promise, resolve } = Promise.withResolvers<void>();
  const ws = new WebSocket("ws://localhost:4242");
  ws.onerror = () => fail();
  ws.onopen = () => ws.send("foo");
  ws.onmessage = (e) => {
    assertEquals(e.data, "foo");
    ws.close();
  };
  ws.onclose = () => {
    resolve();
  };
  await promise;
});

Deno.test("echo string tls", async () => {
  const deferred1 = Promise.withResolvers<void>();
  const deferred2 = Promise.withResolvers<void>();
  const ws = new WebSocket("wss://localhost:4243");
  ws.onerror = () => fail();
  ws.onopen = () => ws.send("foo");
  ws.onmessage = (e) => {
    assertEquals(e.data, "foo");
    ws.close();
    deferred1.resolve();
  };
  ws.onclose = () => {
    deferred2.resolve();
  };
  await deferred1.promise;
  await deferred2.promise;
});

Deno.test("websocket error", async () => {
  const { promise, resolve } = Promise.withResolvers<void>();
  const ws = new WebSocket("wss://localhost:4242");
  ws.onopen = () => fail();
  ws.onerror = (err) => {
    assert(err instanceof ErrorEvent);
    assertEquals(
      err.message,
      "NetworkError: failed to connect to WebSocket: received corrupt message of type InvalidContentType",
    );
    resolve();
  };
  await promise;
});

Deno.test("echo blob with binaryType blob", async () => {
  const { promise, resolve } = Promise.withResolvers<void>();
  const ws = new WebSocket("ws://localhost:4242");
  const blob = new Blob(["foo"]);
  ws.onerror = () => fail();
  ws.onopen = () => ws.send(blob);
  ws.onmessage = (e) => {
    e.data.text().then((actual: string) => {
      blob.text().then((expected) => {
        assertEquals(actual, expected);
      });
    });
    ws.close();
  };
  ws.onclose = () => {
    resolve();
  };
  await promise;
});

Deno.test("echo blob with binaryType arraybuffer", async () => {
  const { promise, resolve } = Promise.withResolvers<void>();
  const ws = new WebSocket("ws://localhost:4242");
  ws.binaryType = "arraybuffer";
  const blob = new Blob(["foo"]);
  ws.onerror = () => fail();
  ws.onopen = () => ws.send(blob);
  ws.onmessage = (e) => {
    blob.arrayBuffer().then((expected) => {
      assertEquals(e.data, expected);
    });
    ws.close();
  };
  ws.onclose = () => {
    resolve();
  };
  await promise;
});

Deno.test("echo uint8array with binaryType blob", async () => {
  const { promise, resolve } = Promise.withResolvers<void>();
  const ws = new WebSocket("ws://localhost:4242");
  const uint = new Uint8Array([102, 111, 111]);
  ws.onerror = () => fail();
  ws.onopen = () => ws.send(uint);
  ws.onmessage = (e) => {
    e.data.arrayBuffer().then((actual: ArrayBuffer) => {
      assertEquals(actual, uint.buffer);
    });
    ws.close();
  };
  ws.onclose = () => {
    resolve();
  };
  await promise;
});

Deno.test("echo uint8array with binaryType arraybuffer", async () => {
  const { promise, resolve } = Promise.withResolvers<void>();
  const ws = new WebSocket("ws://localhost:4242");
  ws.binaryType = "arraybuffer";
  const uint = new Uint8Array([102, 111, 111]);
  ws.onerror = () => fail();
  ws.onopen = () => ws.send(uint);
  ws.onmessage = (e) => {
    assertEquals(e.data, uint.buffer);
    ws.close();
  };
  ws.onclose = () => {
    resolve();
  };
  await promise;
});

Deno.test("echo arraybuffer with binaryType blob", async () => {
  const { promise, resolve } = Promise.withResolvers<void>();
  const ws = new WebSocket("ws://localhost:4242");
  const buffer = new ArrayBuffer(3);
  ws.onerror = () => fail();
  ws.onopen = () => ws.send(buffer);
  ws.onmessage = (e) => {
    e.data.arrayBuffer().then((actual: ArrayBuffer) => {
      assertEquals(actual, buffer);
    });
    ws.close();
  };
  ws.onclose = () => {
    resolve();
  };
  await promise;
});

Deno.test("echo arraybuffer with binaryType arraybuffer", async () => {
  const { promise, resolve } = Promise.withResolvers<void>();
  const ws = new WebSocket("ws://localhost:4242");
  ws.binaryType = "arraybuffer";
  const buffer = new ArrayBuffer(3);
  ws.onerror = () => fail();
  ws.onopen = () => ws.send(buffer);
  ws.onmessage = (e) => {
    assertEquals(e.data, buffer);
    ws.close();
  };
  ws.onclose = () => {
    resolve();
  };
  await promise;
});

Deno.test("echo blob mixed with string", async () => {
  const { promise, resolve } = Promise.withResolvers<void>();
  const ws = new WebSocket("ws://localhost:4242");
  ws.binaryType = "arraybuffer";
  const blob = new Blob(["foo"]);
  ws.onerror = () => fail();
  ws.onopen = () => {
    ws.send(blob);
    ws.send("bar");
  };
  const messages: (ArrayBuffer | string)[] = [];
  ws.onmessage = (e) => {
    messages.push(e.data);
    if (messages.length === 2) {
      assertEquals(messages[0], new Uint8Array([102, 111, 111]).buffer);
      assertEquals(messages[1], "bar");
      ws.close();
    }
  };
  ws.onclose = () => {
    resolve();
  };
  await promise;
});

Deno.test("Event Handlers order", async () => {
  const { promise, resolve } = Promise.withResolvers<void>();
  const ws = new WebSocket("ws://localhost:4242");
  const arr: number[] = [];
  ws.onerror = () => fail();
  ws.addEventListener("message", () => arr.push(1));
  ws.onmessage = () => fail();
  ws.addEventListener("message", () => {
    arr.push(3);
    ws.close();
    assertEquals(arr, [1, 2, 3]);
  });
  ws.onmessage = () => arr.push(2);
  ws.onopen = () => ws.send("Echo");
  ws.onclose = () => {
    resolve();
  };
  await promise;
});

Deno.test("Close without frame", async () => {
  const { promise, resolve } = Promise.withResolvers<void>();
  const ws = new WebSocket("ws://localhost:4244");
  ws.onerror = () => fail();
  ws.onclose = (e) => {
    assertEquals(e.code, 1005);
    resolve();
  };
  await promise;
});

Deno.test("Close connection", async () => {
  const ac = new AbortController();
  const listeningDeferred = Promise.withResolvers<void>();

  const server = Deno.serve({
    handler: (req) => {
      const { socket, response } = Deno.upgradeWebSocket(req);
      socket.onmessage = function (e) {
        socket.close(1008);
        assertEquals(e.data, "Hello");
      };
      socket.onclose = () => {
        ac.abort();
      };
      socket.onerror = () => fail();
      return response;
    },
    signal: ac.signal,
    onListen: () => listeningDeferred.resolve(),
    hostname: "localhost",
    port: servePort,
  });

  await listeningDeferred.promise;

  const conn = await Deno.connect({ port: servePort, hostname: "localhost" });
  await conn.write(
    new TextEncoder().encode(
      "GET / HTTP/1.1\r\nConnection: Upgrade\r\nUpgrade: websocket\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\n\r\n",
    ),
  );

  // Write a 2 text frame saying "Hello"
  await conn.write(new Uint8Array([0x81, 0x05]));
  await conn.write(new TextEncoder().encode("Hello"));

  // We are a bad client so we won't acknowledge the close frame
  await conn.write(new Uint8Array([0x81, 0x05]));
  await conn.write(new TextEncoder().encode("Hello"));

  await server.finished;
  conn.close();
});
