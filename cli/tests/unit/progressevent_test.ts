// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
import { assertEquals } from "./test_util.ts";

Deno.test(function progressEventConstruct() {
  const progressEventDefs = new ProgressEvent("progressEventType", {});
  assertEquals(progressEventDefs.lengthComputable, false);
  assertEquals(progressEventDefs.loaded, 0);
  assertEquals(progressEventDefs.total, 0);

  const progressEvent = new ProgressEvent("progressEventType", {
    lengthComputable: true,
    loaded: 123,
    total: 456,
  });
  assertEquals(progressEvent.lengthComputable, true);
  assertEquals(progressEvent.loaded, 123);
  assertEquals(progressEvent.total, 456);
});
