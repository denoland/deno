// Copyright 2018-2025 the Deno authors. MIT license.
import { assertEquals } from "@std/assert";

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
