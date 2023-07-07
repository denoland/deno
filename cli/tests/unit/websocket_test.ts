// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import {
  assert,
  assertEquals,
  assertThrows,
  deferred,
  fail,
} from "./test_util.ts";

Deno.test({ permissions: "none" }, function websocketPermissionless() {
  assertThrows(
    () => new WebSocket("ws://localhost"),
    Deno.errors.PermissionDenied,
  );
});

Deno.test(async function websocketConstructorTakeURLObjectAsParameter() {
  const promise = deferred();
  const ws = new WebSocket(new URL("ws://localhost:4242/"));
  assertEquals(ws.url, "ws://localhost:4242/");
  ws.onerror = () => fail();
  ws.onopen = () => ws.close();
  ws.onclose = () => {
    promise.resolve();
  };
  await promise;
});

// https://github.com/denoland/deno/pull/17762
// https://github.com/denoland/deno/issues/17761
Deno.test(async function websocketPingPong() {
  const promise = deferred();
  const ws = new WebSocket("ws://localhost:4245/");
  assertEquals(ws.url, "ws://localhost:4245/");
  ws.onerror = () => fail();
  ws.onmessage = (e) => {
    ws.send(e.data);
  };
  ws.onclose = () => {
    promise.resolve();
  };
  await promise;
  ws.close();
});

// TODO(mmastrac): This requires us to ignore bad certs
// Deno.test(async function websocketSecureConnect() {
//   const promise = deferred();
//   const ws = new WebSocket("wss://localhost:4243/");
//   assertEquals(ws.url, "wss://localhost:4243/");
//   ws.onerror = (error) => {
//     console.log(error);
//     fail();
//   };
//   ws.onopen = () => ws.close();
//   ws.onclose = () => {
//     promise.resolve();
//   };
//   await promise;
// });

// https://github.com/denoland/deno/issues/18700
Deno.test(
  { sanitizeOps: false, sanitizeResources: false },
  async function websocketWriteLock() {
    const ac = new AbortController();
    const listeningPromise = deferred();

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
      onListen: () => listeningPromise.resolve(),
      hostname: "localhost",
      port: 4246,
    });

    await listeningPromise;
    const promise = deferred();
    const ws = new WebSocket("ws://localhost:4246/");
    assertEquals(ws.url, "ws://localhost:4246/");
    ws.onerror = () => fail();
    ws.onmessage = (e) => {
      assertEquals(e.data, "Hello");
      setTimeout(() => {
        ws.send(e.data);
      }, 1000);
      promise.resolve();
    };
    ws.onclose = () => {
      promise.resolve();
    };

    await Promise.all([promise, server.finished]);
    ws.close();
  },
);

// https://github.com/denoland/deno/issues/18775
Deno.test({
  sanitizeOps: false,
  sanitizeResources: false,
}, async function websocketDoubleClose() {
  const promise = deferred();

  const ac = new AbortController();
  const listeningPromise = deferred();

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
    onListen: () => listeningPromise.resolve(),
    hostname: "localhost",
    port: 4247,
  });

  await listeningPromise;

  const ws = new WebSocket("ws://localhost:4247/");
  assertEquals(ws.url, "ws://localhost:4247/");
  ws.onerror = () => fail();
  ws.onmessage = () => ws.send("bye");
  ws.onclose = () => {
    promise.resolve();
  };
  await Promise.all([promise, server.finished]);
});

// https://github.com/denoland/deno/issues/19483
Deno.test({
  sanitizeOps: false,
  sanitizeResources: false,
}, async function websocketCloseFlushes() {
  const promise = deferred();

  const ac = new AbortController();
  const listeningPromise = deferred();

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
    onListen: () => listeningPromise.resolve(),
    hostname: "localhost",
    port: 4247,
  });

  await listeningPromise;

  const ws = new WebSocket("ws://localhost:4247/");
  assertEquals(ws.url, "ws://localhost:4247/");
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
    promise.resolve();
  };
  await Promise.all([promise, server.finished]);

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
