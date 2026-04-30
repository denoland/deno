// Copyright 2018-2026 the Deno authors. MIT license.
import { register } from "checkin:loader";

// Register a resolve hook that maps virtual:greeting to a real module
register({
  async resolve(specifier, _context, nextResolve) {
    if (specifier === "virtual:greeting") {
      return { url: "test:///integration/loader_resolve/greeting.ts" };
    }
    return nextResolve(specifier);
  },
});

// Dynamic import triggers the async resolve path
const mod = await import("virtual:greeting");
console.log(mod.default);
