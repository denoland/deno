import dep from "@denotest/different-nested-dep";

// `@denotest/different-nested-dep@1.0.0` declares a dependency on
// `@denotest/different-nested-dep-child@1.0.0` (which exports `1`). The
// `scopes` entry in deno.json keyed by `npm:@denotest/different-nested-dep@1.0.0`
// remaps that transitive dependency to `2.0.0` (which exports `2`), so this
// prints `2` instead of `1`.
console.log(dep);
