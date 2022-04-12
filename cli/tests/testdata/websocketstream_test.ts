// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

import {
  assert,
  assertEquals,
  assertNotEquals,
  assertRejects,
  assertThrows,
  unreachable,
} from "../../../test_util/std/testing/asserts.ts";

Deno.test("fragment", () => {
  assertThrows(() => new WebSocketStream("ws://localhost:4242/#"));
  assertThrows(() => new WebSocketStream("ws://localhost:4242/#foo"));
});

Deno.test("duplicate protocols", () => {
  assertThrows(() =>
    new WebSocketStream("ws://localhost:4242", {
      protocols: ["foo", "foo"],
    })
  );
});

Deno.test("connect & close custom valid code", async () => {
  const ws = new WebSocketStream("ws://localhost:4242");
  await ws.connection;
  ws.close({ code: 1000 });
  await ws.closed;
});

Deno.test("connect & close custom invalid reason", async () => {
  const ws = new WebSocketStream("ws://localhost:4242");
  await ws.connection;
  assertThrows(() => ws.close({ code: 1000, reason: "".padEnd(124, "o") }));
  ws.close();
  await ws.closed;
});

Deno.test("echo string", async () => {
  const ws = new WebSocketStream("ws://localhost:4242");
  const { readable, writable } = await ws.connection;
  await writable.getWriter().write("foo");
  const res = await readable.getReader().read();
  assertEquals(res.value, "foo");
  ws.close();
  await ws.closed;
});

Deno.test("echo string tls", async () => {
  const ws = new WebSocketStream("wss://localhost:4243");
  const { readable, writable } = await ws.connection;
  await writable.getWriter().write("foo");
  const res = await readable.getReader().read();
  assertEquals(res.value, "foo");
  ws.close();
  await ws.closed;
});

Deno.test("websocket error", async () => {
  const ws = new WebSocketStream("wss://localhost:4242");
  await Promise.all([
    assertRejects(
      () => ws.connection,
      Deno.errors.UnexpectedEof,
      "tls handshake eof",
    ),
    assertRejects(
      () => ws.closed,
      Deno.errors.UnexpectedEof,
      "tls handshake eof",
    ),
  ]);
});

Deno.test("echo uint8array", async () => {
  const ws = new WebSocketStream("ws://localhost:4242");
  const { readable, writable } = await ws.connection;
  const uint = new Uint8Array([102, 111, 111]);
  await writable.getWriter().write(uint);
  const res = await readable.getReader().read();
  assertEquals(res.value, uint);
  ws.close();
  await ws.closed;
});

Deno.test("aborting immediately throws an AbortError", async () => {
  const controller = new AbortController();
  const wss = new WebSocketStream("ws://localhost:4242", {
    signal: controller.signal,
  });
  controller.abort();
  await assertRejects(
    () => wss.connection,
    (error: Error) => {
      assert(error instanceof DOMException);
      assertEquals(error.name, "AbortError");
    },
  );
  await assertRejects(
    () => wss.closed,
    (error: Error) => {
      assert(error instanceof DOMException);
      assertEquals(error.name, "AbortError");
    },
  );
});

Deno.test("aborting immediately with a reason throws that reason", async () => {
  const controller = new AbortController();
  const wss = new WebSocketStream("ws://localhost:4242", {
    signal: controller.signal,
  });
  const abortReason = new Error();
  controller.abort(abortReason);
  await assertRejects(
    () => wss.connection,
    (error: Error) => assertEquals(error, abortReason),
  );
  await assertRejects(
    () => wss.closed,
    (error: Error) => assertEquals(error, abortReason),
  );
});

Deno.test("aborting immediately with a primitive as reason throws that primitive", async () => {
  const controller = new AbortController();
  const wss = new WebSocketStream("ws://localhost:4242", {
    signal: controller.signal,
  });
  controller.abort("Some string");
  await wss.connection.then(
    () => unreachable(),
    (e) => assertEquals(e, "Some string"),
  );
  await wss.closed.then(
    () => unreachable(),
    (e) => assertEquals(e, "Some string"),
  );
});

Deno.test("headers", async () => {
  const listener = Deno.listen({ port: 4512 });
  const promise = (async () => {
    const conn = await listener.accept();
    const httpConn = Deno.serveHttp(conn);
    const { request, respondWith } = (await httpConn.nextRequest())!;
    assertEquals(request.headers.get("x-some-header"), "foo");
    const { response, socket } = Deno.upgradeWebSocket(request);
    socket.onopen = () => socket.close();
    const p = new Promise<void>((resolve) => {
      socket.onopen = () => socket.close();
      socket.onclose = () => resolve();
    });
    await respondWith(response);
    await p;
  })();

  const ws = new WebSocketStream("ws://localhost:4512", {
    headers: [["x-some-header", "foo"]],
  });
  await ws.connection;
  await promise;
  await ws.closed;
  listener.close();
});

Deno.test("forbidden headers", async () => {
  const forbiddenHeaders = [
    "sec-websocket-accept",
    "sec-websocket-extensions",
    "sec-websocket-key",
    "sec-websocket-protocol",
    "sec-websocket-version",
    "upgrade",
    "connection",
  ];

  const listener = Deno.listen({ port: 4512 });
  const promise = (async () => {
    const conn = await listener.accept();
    const httpConn = Deno.serveHttp(conn);
    const { request, respondWith } = (await httpConn.nextRequest())!;
    for (const [key] of request.headers) {
      assertNotEquals(key, "foo");
    }
    const { response, socket } = Deno.upgradeWebSocket(request);
    const p = new Promise<void>((resolve) => {
      socket.onopen = () => socket.close();
      socket.onclose = () => resolve();
    });
    await respondWith(response);
    await p;
  })();

  const ws = new WebSocketStream("ws://localhost:4512", {
    headers: forbiddenHeaders.map((header) => [header, "foo"]),
  });
  await ws.connection;
  await promise;
  await ws.closed;
  listener.close();
});
