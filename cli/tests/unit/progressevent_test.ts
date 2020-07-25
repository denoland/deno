// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { unitTest, assertEquals } from "./test_util.ts";

unitTest(function progressEventConstruct(): void {
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
