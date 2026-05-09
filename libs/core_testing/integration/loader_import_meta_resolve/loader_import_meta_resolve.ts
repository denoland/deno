// Copyright 2018-2026 the Deno authors. MIT license.

// Test that import.meta.resolve() invokes registered loader hooks,
// matching Node.js behavior where import.meta.resolve() goes through
// the loader hook chain (see test-esm-import-meta-resolve-hooks.mjs).

import { register } from "checkin:loader";

register({
  async resolve(specifier, _context, nextResolve) {
    if (specifier === "custom:hooked") {
      return {
        url: "test:///integration/loader_import_meta_resolve/hooked.js",
      };
    }
    return nextResolve(specifier);
  },
});

// import.meta.resolve should invoke the resolve hook and return the
// remapped URL, not the original specifier.
const resolved = import.meta.resolve("custom:hooked");
console.log(resolved);
