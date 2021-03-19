// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import { assertEquals } from "./test_util.ts";

Deno.test("symlinkSyncPerm", async function () {
  const rs = new ReadableStream<string>({
    start(controller) {
      controller.enqueue("hello ");
      controller.enqueue("deno");
      controller.close();
    },
  });

  for await (const chunk of rs.getIterator()) {
    assertEquals(typeof chunk, "string");
  }
});
