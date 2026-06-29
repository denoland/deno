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
