// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

import {
  assertEquals,
  assertThrows,
  assertThrowsAsync,
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
    assertThrowsAsync(
      () => ws.connection,
      Deno.errors.UnexpectedEof,
      "tls handshake eof",
    ),
    assertThrowsAsync(
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
