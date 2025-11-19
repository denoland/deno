/// <reference path="./cli/tsc/dts/lib.dom.d.ts" />
/// <reference path="./cli/tsc/dts/lib.deno_cache.d.ts" />

// Try to use Cache to ensure types are compatible and constructible (as per the fix)
const c = new Cache();
const cs = new CacheStorage();
