// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { assertEquals, assertThrows, deferred, fail } from "./test_util.ts";

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

    await Promise.all([promise, server]);
    ws.close();
  },
);
