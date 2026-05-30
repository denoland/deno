// Regression test for https://github.com/denoland/deno/issues/25189
//
// In global-cache mode (no --node-modules-dir), `require()` of a top-level
// npm dependency must succeed even when the referrer module lives outside
// of the global cache. This mirrors the Playwright scenario described in
// the issue: a package in the cache installs a `require()` hook that
// transpiles a user-project file to CJS, and the transpiled code re-issues
// `require("pkg")` from that user-project file as its parent.
import "npm:@denotest/add";
import "npm:@denotest/cjs-multiple-exports";
import { createRequire } from "node:module";

// `createRequire(import.meta.url)` produces a require whose referrer is
// this very module — a file in the user's project directory, not in
// DENODIR. Without the global-cache fallback, the bare-specifier lookups
// below would throw `Cannot find module`.
const require = createRequire(import.meta.url);

const add = require("@denotest/add").add;
console.log("add(2, 3) =", add(2, 3));

// Subpath into a package — the fallback must extract the package name
// ("@denotest/cjs-multiple-exports") and let _findPath resolve the
// subpath ("/add") inside the package folder.
const addSubpath = require("@denotest/cjs-multiple-exports/add");
console.log("subpath add(2, 3) =", addSubpath(2, 3));
