// Copyright 2018-2026 the Deno authors. MIT license.

import { loadTestLibrary } from "./common.js";

const lib = loadTestLibrary();

// Signal that the addon is loaded
self.postMessage("ready");

self.onmessage = (e) => {
  if (e.data === "create_externals") {
    // Create an external buffer -- its C finalizer must not crash
    // when the worker is terminated before GC runs.
    lib.test_external_buffer();
    self.postMessage("created");
  }
};
