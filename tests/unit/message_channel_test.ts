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

Deno.test("messagechannel single-listener dispatch fast path event state", async () => {
  // With a single `message` listener and no transferables the dispatch takes
  // the fast path that invokes the handler directly. The event the handler sees
  // must still be spec-correct: `target`/`currentTarget` are the port,
  // `eventPhase` is AT_TARGET during the call, `composedPath()` is `[port]`, and
  // the dispatch state is reset afterwards.
  const mc = new MessageChannel();
  const { promise, resolve } = Promise.withResolvers<void>();
  let event: MessageEvent;
  mc.port2.onmessage = (e) => {
    event = e;
    assertEquals(e.target, mc.port2);
    assertEquals(e.currentTarget, mc.port2);
    assertEquals(e.eventPhase, Event.AT_TARGET);
    assertEquals(e.composedPath(), [mc.port2]);
    assert(e.isTrusted);
    resolve();
  };
  mc.port1.postMessage("hello");
  await promise;
  // After dispatch the event state is reset.
  assertEquals(event!.currentTarget, null);
  assertEquals(event!.eventPhase, Event.NONE);
  assertEquals(event!.composedPath(), []);
  mc.port1.close();
  mc.port2.close();
});

Deno.test("messagechannel multiple message listeners all fire (dispatch fallback)", async () => {
  // More than one `message` listener forces the full event-dispatch machinery
  // (the fast path only handles the single-listener case). All listeners, in
  // registration order, must run.
  const mc = new MessageChannel();
  const { promise, resolve } = Promise.withResolvers<void>();
  const order: number[] = [];
  mc.port2.addEventListener("message", () => order.push(1));
  mc.port2.onmessage = () => order.push(2);
  mc.port2.addEventListener("message", () => {
    order.push(3);
    resolve();
  });
  mc.port1.postMessage("hello");
  await promise;
  assertEquals(order, [1, 2, 3]);
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
