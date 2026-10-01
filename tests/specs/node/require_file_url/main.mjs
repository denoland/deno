// Bundlers that emit ESM with a `createRequire()` shim (rolldown, esbuild)
// rewrite externalized dependencies to absolute `file://` specifiers. This is
// how a CommonJS Vite config ends up calling `require("file:///.../dep.mjs")`,
// since Vite always bundles the config to ESM under Deno.
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";

const require = createRequire(import.meta.url);
const fileUrl = (path) => new URL(path, import.meta.url).href;

// An ESM entry required through a `file://` URL -- the failing case in #35457.
console.log(require(fileUrl("./esm-dep.mjs")).default);
// A CommonJS file through a `file://` URL.
console.log(require(fileUrl("./cjs-dep.js")));
// Extension resolution.
console.log(require(fileUrl("./cjs-dep")));
// Directory index resolution.
console.log(require(fileUrl("./dir")));
// Directory package.json "main" resolution.
console.log(require(fileUrl("./pkg-dir")));
// Percent-encoded characters are decoded.
console.log(require(fileUrl("./with space.js")));

// The URL and the path resolve to the same module.
console.log(
  require.resolve(fileUrl("./cjs-dep.js")) ===
    fileURLToPath(fileUrl("./cjs-dep.js")),
);
console.log(require(fileUrl("./cjs-dep.js")) === require("./cjs-dep.js"));

// A query or fragment has no meaning for CommonJS module identity, so it is
// not resolved rather than silently sharing the cache entry of the bare path.
for (const suffix of ["?t=1", "#frag"]) {
  try {
    require(fileUrl("./cjs-dep.js") + suffix);
    console.log("unexpectedly resolved", suffix);
  } catch (err) {
    console.log(suffix, err.code);
  }
}

// A `file://` URL that doesn't exist still reports MODULE_NOT_FOUND.
try {
  require(fileUrl("./missing.js"));
} catch (err) {
  console.log("missing", err.code);
}
