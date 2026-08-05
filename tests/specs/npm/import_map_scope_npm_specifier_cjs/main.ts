import dep from "parent";

// `@denotest/cjs-different-nested-dep@1.0.0` is a CommonJS package that does
// `require("@denotest/different-nested-dep-child")`. Its dependency range
// (`>=1.0.0`) would normally resolve the child to `2.0.0` (exporting `2`).
//
// The `scopes` entry in deno.json keyed by
// `npm:@denotest/cjs-different-nested-dep@1.0.0` overrides that transitive
// dependency to `1.0.0` (exporting `1`). Even though the `require()` itself is
// resolved by node resolution (not the import-map-aware resolver), the override
// is seeded into the npm snapshot, so the package's CJS `require()` resolves to
// the overridden version and this prints `1` instead of `2`.
console.log(dep);
