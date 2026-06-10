// Copyright 2018-2026 the Deno authors. MIT license.
import { assert, assertEquals } from "@std/assert";

Deno.test("BroadcastChannel worker", async () => {
  const intercom = new BroadcastChannel("intercom");
  let count = 0;

  const url = import.meta.resolve(
    "../testdata/workers/broadcast_channel.ts",
  );
  const worker = new Worker(url, { type: "module", name: "worker" });
  worker.onmessage = () => intercom.postMessage(++count);

  const { promise, resolve } = Promise.withResolvers<void>();

  intercom.onmessage = function (e) {
    assertEquals(count, e.data);
    if (count < 42) {
      intercom.postMessage(++count);
    } else {
      worker.terminate();
      intercom.close();
      resolve();
    }
  };

  await promise;
});

Deno.test("BroadcastChannel immediate close after post", () => {
  const bc = new BroadcastChannel("internal_notification");
  bc.postMessage("New listening connected!");
  bc.close();
});

// Regression test for https://github.com/denoland/deno/issues/34836
// A SharedArrayBuffer posted to a BroadcastChannel must be deserializable by
// *every* receiver, not just the first one. Here two channels in the same VM
// listen on the same name, so the message fans out to both.
Deno.test("BroadcastChannel SharedArrayBuffer to multiple receivers", async () => {
  const sab = new SharedArrayBuffer(4);
  new Uint32Array(sab)[0] = 12345;

  const sender = new BroadcastChannel("sab");
  const a = new BroadcastChannel("sab");
  const b = new BroadcastChannel("sab");

  const { promise, resolve } = Promise.withResolvers<void>();
  let received = 0;
  const onmessage = (e: MessageEvent) => {
    assert(e.data instanceof SharedArrayBuffer);
    assertEquals(e.data.byteLength, 4);
    // The buffer is genuinely shared, not copied: writes made by the sender
    // after posting are visible to the receivers.
    assertEquals(new Uint32Array(e.data)[0], 54321);
    if (++received === 2) resolve();
  };
  a.onmessage = onmessage;
  b.onmessage = onmessage;

  sender.postMessage(sab);
  // Mutate after posting; because delivery is deferred and the buffer is
  // shared, the receivers observe this write.
  new Uint32Array(sab)[0] = 54321;

  await promise;
  assertEquals(received, 2);

  sender.close();
  a.close();
  b.close();
});
