// Copyright 2018-2026 the Deno authors. MIT license.

import { loadTestLibrary } from "./common.js";

const lib = loadTestLibrary();

// Signal that the addon is loaded
self.postMessage("ready");

self.onmessage = (e) => {
  if (e.data === "block_tsfn_queue") {
    lib.test_tsfn_blocking_full_queue();
    while (lib.tsfn_full_queue_state() < 1) { /* first call queued */ }
    self.postMessage("blocked");
    // Never drain the queue: the execute thread's second call stays blocked.
    while (true) { /* interrupted by the parent */ }
  }
  if (e.data === "create_externals") {
    // Create an external buffer -- its C finalizer must not crash
    // when the worker is terminated before GC runs.
    lib.test_external_buffer();
    self.postMessage("created");
  }
};
