// Copyright 2018-2026 the Deno authors. MIT license.
import { barrierAwait, barrierCreate } from "checkin:async";

barrierCreate("barrier", 2);
(async () => {
  await import("./dynamic.js");
  await barrierAwait("barrier");
})();
