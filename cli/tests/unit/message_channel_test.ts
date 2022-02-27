// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
// NOTE: these are just sometests to test the TypeScript types. Real coverage is
// provided by WPT.
import {
  assert,
  assertEquals,
} from "../../../test_util/std/testing/asserts.ts";
import { deferred } from "../../../test_util/std/async/deferred.ts";

Deno.test("messagechannel", async () => {
  const mc = new MessageChannel();
  const mc2 = new MessageChannel();
  assert(mc.port1);
  assert(mc.port2);

  const promise = deferred();

  mc.port2.onmessage = (e) => {
    assertEquals(e.data, "hello");
    assertEquals(e.ports.length, 1);
    assert(e.ports[0] instanceof MessagePort);
    e.ports[0].close();
    promise.resolve();
  };

  mc.port1.postMessage("hello", [mc2.port1]);
  mc.port1.close();

  await promise;

  mc.port2.close();
  mc2.port2.close();
});

Deno.test("messagechannel clone port", async () => {
  const mc = new MessageChannel();
  const mc2 = new MessageChannel();
  assert(mc.port1);
  assert(mc.port2);

  const promise = deferred();

  mc.port2.onmessage = (e) => {
    const { port } = e.data;
    assertEquals(e.ports.length, 1);
    assert(e.ports[0] instanceof MessagePort);
    assertEquals(e.ports[0], port);
    e.ports[0].close();
    promise.resolve();
  };

  mc.port1.postMessage({ port: mc2.port1 }, [mc2.port1]);
  mc.port1.close();

  await promise;

  mc.port2.close();
  mc2.port2.close();
});
