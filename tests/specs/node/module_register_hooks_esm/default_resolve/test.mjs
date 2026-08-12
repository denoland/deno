// Regression test for https://github.com/denoland/deno/pull/34081
// review-4305273060: when the resolve-hook chain calls `nextResolve()` at the
// terminal step, it must return the URL Deno's real ESM resolver would
// produce — not a stub from `new URL(spec, parentURL)` or a CJS
// `_resolveFilename` lookup. This is what Node does, and is required for
// hooks that wrap or inspect the default resolution of bare specifiers,
// package subpaths, import-map aliases, etc.

import { createRequire } from "node:module";
const require = createRequire(import.meta.url);
const { registerHooks } = require("module");

const seen = [];

const hook = registerHooks({
  resolve(specifier, context, nextResolve) {
    const result = nextResolve(specifier, context);
    seen.push({ specifier, url: result.url });
    return result;
  },
});

const aliased = await import("#aliased");
const relative = await import("./target.mjs");

hook.deregister();

const aliasedEntry = seen.find((e) => e.specifier === "#aliased");
const relativeEntry = seen.find((e) => e.specifier === "./target.mjs");

console.log("aliased value:", aliased.value);
console.log("relative value:", relative.value);
console.log(
  "aliased url resolves through deno:",
  typeof aliasedEntry?.url === "string" &&
    aliasedEntry.url.endsWith("/target.mjs"),
);
console.log(
  "relative url resolves through deno:",
  typeof relativeEntry?.url === "string" &&
    relativeEntry.url.endsWith("/target.mjs"),
);
