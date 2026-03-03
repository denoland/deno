// Copyright 2018-2026 the Deno authors. MIT license.
import { registerResolveMapping } from "checkin:loader";

// Register a virtual specifier that maps to a real module via async resolution
registerResolveMapping(
  "virtual:greeting",
  "test:///integration/loader_resolve/greeting.ts",
);

// Dynamic import triggers the async resolve path
const mod = await import("virtual:greeting");
console.log(mod.default);
