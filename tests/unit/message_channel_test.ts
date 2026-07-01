// Copyright 2018-2026 the Deno authors. MIT license.
// NOTE: these are just sometests to test the TypeScript types. Real coverage is
// provided by WPT.
import { assert, assertEquals } from "@std/assert";

Deno.test("messagechannel", async () => {
  const mc = new MessageChannel();
  const mc2 = new MessageChannel();
  assert(mc.port1);
  assert(mc.port2);

  const { promise, resolve } = Promise.withResolvers<void>();

  mc.port2.onmessage = (e) => {
    assertEquals(e.data, "hello");
    assertEquals(e.ports.length, 1);
    assert(e.ports[0] instanceof MessagePort);
    e.ports[0].close();
    resolve();
  };

  mc.port1.postMessage("hello", [mc2.port1]);
  mc.port1.close();

  await promise;

  mc.port2.close();
  mc2.port2.close();
});

Deno.test("messagechannel no-transferables ports is empty frozen array", async () => {
  // Covers the dispatch fast path (no transferables -> the MessageEvent
  // `ports` is a single frozen empty array, with no per-message filter) and
  // the recv op returning its payload without the serde object round-trip.
  const mc = new MessageChannel();
  const { promise, resolve } = Promise.withResolvers<void>();
  const received: MessageEvent[] = [];
  mc.port2.onmessage = (e) => {
    received.push(e);
    if (received.length === 3) resolve();
  };
  mc.port1.postMessage("a");
  mc.port1.postMessage({ x: 1 });
  mc.port1.postMessage(42);
  await promise;
  assertEquals(received.map((e) => e.data), ["a", { x: 1 }, 42]);
  for (const e of received) {
    assert(Array.isArray(e.ports));
    assertEquals(e.ports.length, 0);
    assert(Object.isFrozen(e.ports));
  }
  mc.port1.close();
  mc.port2.close();
});

Deno.test("messagechannel primitive fast path", async () => {
  // Primitives take a custom encoding that bypasses V8's structured-clone
  // serializer; verify a representative spread round-trips exactly, including
  // the tricky cases (-0, NaN, +/-Infinity, int32 boundaries, strings with
  // lone surrogates) and a bigint that intentionally falls back to V8.
  const mc = new MessageChannel();
  const values: unknown[] = [
    undefined,
    null,
    true,
    false,
    0,
    -0,
    1,
    -1,
    42,
    -42,
    2147483647, // int32 max
    -2147483648, // int32 min
    2147483648, // just past int32 -> double path
    -2147483649,
    0.5,
    -0.5,
    3.141592653589793,
    1e308,
    -1e308,
    Number.MAX_SAFE_INTEGER,
    Number.MIN_SAFE_INTEGER,
    Infinity,
    -Infinity,
    NaN,
    "",
    "hello world",
    "\uD800",
    "\uDC00",
    "\uD83D\uDE00",
    123n, // bigint -> V8 fallback
  ];
  // Expected received values. These match `values` except for `undefined`:
  // dispatch builds the delivered event via `new MessageEvent("message",
  // { data })`, and a WebIDL dictionary member whose value is `undefined`
  // falls back to its default, which for `MessageEvent.data` is `null`. So
  // posting `undefined` is observably delivered as `null` -- this is
  // pre-existing Deno behavior, independent of the fast path, and the fast
  // path must preserve it.
  const expected = values.map((v) => v === undefined ? null : v);

  const received: unknown[] = [];
  const { promise, resolve } = Promise.withResolvers<void>();
  mc.port2.onmessage = (e) => {
    received.push(e.data);
    if (received.length === values.length) resolve();
  };

  for (let i = 0; i < values.length; i++) {
    mc.port1.postMessage(values[i]);
  }

  await promise;

  assertEquals(received.length, expected.length);
  for (let i = 0; i < expected.length; i++) {
    // Object.is distinguishes -0/+0 and treats NaN as equal to itself.
    assert(
      Object.is(received[i], expected[i]),
      `index ${i}: got ${String(received[i])}, expected ${String(expected[i])}`,
    );
  }

  mc.port1.close();
  mc.port2.close();
});

Deno.test("messagechannel clone port", async () => {
  const mc = new MessageChannel();
  const mc2 = new MessageChannel();
  assert(mc.port1);
  assert(mc.port2);

  const { promise, resolve } = Promise.withResolvers<void>();

  mc.port2.onmessage = (e) => {
    const { port } = e.data;
    assertEquals(e.ports.length, 1);
    assert(e.ports[0] instanceof MessagePort);
    assertEquals(e.ports[0], port);
    e.ports[0].close();
    resolve();
  };

  mc.port1.postMessage({ port: mc2.port1 }, [mc2.port1]);
  mc.port1.close();

  await promise;

  mc.port2.close();
  mc2.port2.close();
});
