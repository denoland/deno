// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
import { assert, assertEquals, assertThrows, fail } from "./test_util.ts";

const servePort = 4248;
const serveUrl = `ws://localhost:${servePort}/`;

Deno.test({ permissions: "none" }, function websocketPermissionless() {
  assertThrows(
    () => new WebSocket("ws://localhost"),
    Deno.errors.PermissionDenied,
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
  const cert = await Deno.readTextFile("cli/tests/testdata/tls/localhost.crt");
  const key = await Deno.readTextFile("cli/tests/testdata/tls/localhost.key");

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
