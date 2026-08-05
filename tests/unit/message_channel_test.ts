// Copyright 2018-2026 the Deno authors. MIT license.
// NOTE: these are just sometests to test the TypeScript types. Real coverage is
// provided by WPT.
import { assert, assertEquals, assertThrows } from "@std/assert";

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
  // the tricky cases (-0, NaN, +/-Infinity, int32 boundaries, lone surrogates),
  // every string sub-path (1-byte Latin1, 2-byte, mid-string bail, the length
  // cap and its V8 fallback for long/large strings), and a bigint that
  // intentionally falls back to V8.
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
    "caf\u00E9 \u00FF\u0080", // high-Latin1 code units (128..255) -> 1-byte path
    "abc\u20AC", // starts ASCII then hits a >=256 unit -> bails to 2-byte path
    "x".repeat(128), // ASCII at the fast-path cap -> 1-byte
    "x".repeat(129), // just past the cap -> V8 fallback (still exact)
    "\u20AC".repeat(128), // two-byte at the cap
    "\u20AC".repeat(129), // two-byte past the cap -> V8 fallback
    "x".repeat(40000), // large ASCII -> V8 fallback, exact
    "a".repeat(200) + "\uD83D\uDE00", // large mixed string w/ surrogate pair -> V8, exact
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

Deno.test("messagechannel numeric-array fast path", async () => {
  // Dense all-number arrays take a custom encoding that bypasses V8's
  // structured-clone serializer (int32 elements packed 4 bytes each, otherwise
  // 8-byte f64). Verify a representative spread round-trips exactly and, just as
  // importantly, that arrays which are *not* plain dense number arrays fall back
  // to V8 and keep their full structured-clone semantics (holes, extra own
  // properties, non-number elements, prototype, and the length cap).
  const cap = 4096;
  const bigInts = Array.from({ length: cap }, (_, i) => (i * 2654435761) | 0);
  const pastCap = Array.from({ length: cap + 1 }, (_, i) => i);

  const withHole = [1, 2, 3];
  delete withHole[1];
  const withExtra: number[] & { tag?: string } = [4, 5, 6];
  withExtra.tag = "keep-me";

  const values: unknown[] = [
    [], // empty -> V8 fallback, exact
    [0],
    [1, -1, 42, -42],
    [2147483647, -2147483648], // int32 boundaries
    [2147483648, -2147483649], // just past int32 -> f64 path
    [0.5, -0.5, 3.141592653589793],
    [1, 2.5, 3], // mixed int/float -> f64 path, exact
    [-0, 0, -0], // -0 sign must survive (forces f64 path)
    [NaN, Infinity, -Infinity], // non-finite -> f64 path, exact
    [Number.MAX_SAFE_INTEGER, Number.MIN_SAFE_INTEGER],
    bigInts, // at the fast-path cap
    pastCap, // just past the cap -> V8 fallback, exact
    withHole, // sparse -> V8 fallback, hole preserved
    withExtra, // extra own property -> V8 fallback, property preserved
    ["a", "b"], // strings -> V8 fallback
    [1, "two", 3], // mixed types -> V8 fallback
    [{ a: 1 }, { b: 2 }], // objects -> V8 fallback
    [[1, 2], [3, 4]], // nested arrays -> V8 fallback (elements not numbers)
    [1n, 2n], // bigints -> V8 fallback
  ];

  const received: unknown[] = [];
  const mc = new MessageChannel();
  const { promise, resolve } = Promise.withResolvers<void>();
  mc.port2.onmessage = (e) => {
    received.push(e.data);
    if (received.length === values.length) resolve();
  };
  for (let i = 0; i < values.length; i++) {
    mc.port1.postMessage(values[i]);
  }
  await promise;

  assertEquals(received.length, values.length);
  const eq = (a: unknown, b: unknown): boolean => {
    if (Array.isArray(a) && Array.isArray(b)) {
      if (a.length !== b.length) return false;
      for (let i = 0; i < a.length; i++) {
        if ((i in a) !== (i in b)) return false; // hole-ness must match
        if (!eq(a[i], b[i])) return false;
      }
      // Own enumerable non-index properties must survive too.
      const ak = Object.keys(a), bk = Object.keys(b);
      if (ak.length !== bk.length) return false;
      for (const k of ak) {
        if (!eq((a as never)[k], (b as never)[k])) return false;
      }
      return true;
    }
    if (
      typeof a === "object" && a !== null && typeof b === "object" && b !== null
    ) {
      return JSON.stringify(a) === JSON.stringify(b);
    }
    return Object.is(a, b);
  };
  for (let i = 0; i < values.length; i++) {
    assert(
      eq(received[i], values[i]),
      `index ${i}: got ${Deno.inspect(received[i])}, expected ${
        Deno.inspect(values[i])
      }`,
    );
  }

  mc.port1.close();
  mc.port2.close();
});

Deno.test("messagechannel array-like non-plain-array fall back to V8", () => {
  // A subclass of Array and a Proxy over an array both report
  // `Array.isArray === true`. The fast path must exclude both: a subclass is
  // downgraded by structured clone to a plain Array (still cloneable), while a
  // proxy is not cloneable at all and must throw -- exactly as before this
  // optimization existed.
  const mc = new MessageChannel();
  try {
    class MyArray extends Array {}
    const sub = MyArray.of(1, 2, 3);
    // Cloneable, but not through the fast path (prototype guard) -> no throw.
    mc.port1.postMessage(sub);

    const proxied = new Proxy([1, 2, 3], {});
    assertThrows(() => mc.port1.postMessage(proxied));
  } finally {
    mc.port1.close();
    mc.port2.close();
  }
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
