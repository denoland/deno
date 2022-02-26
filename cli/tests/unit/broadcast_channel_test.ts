// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
import { assertEquals } from "../../../test_util/std/testing/asserts.ts";
import { deferred } from "../../../test_util/std/async/deferred.ts";

Deno.test("broadcastchannel worker", async () => {
  const intercom = new BroadcastChannel("intercom");
  let count = 0;

  const url = new URL(
    "../testdata/workers/broadcast_channel.ts",
    import.meta.url,
  );
  const worker = new Worker(url.href, { type: "module", name: "worker" });
  worker.onmessage = () => intercom.postMessage(++count);

  const promise = deferred();

  intercom.onmessage = function (e) {
    assertEquals(count, e.data);
    if (count < 42) {
      intercom.postMessage(++count);
    } else {
      worker.terminate();
      intercom.close();
      promise.resolve();
    }
  };

  await promise;
});
