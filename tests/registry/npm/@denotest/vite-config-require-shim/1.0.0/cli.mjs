#!/usr/bin/env -S node

import { writeFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const requiredEsmUrl = new URL("./esm-entry.mjs", import.meta.url);
const requiredEsm = fileURLToPath(requiredEsmUrl);
const configPath = join(
  process.cwd(),
  "vite.config.js.timestamp-35457.mjs",
);

writeFileSync(
  configPath,
  `// Regression for https://github.com/denoland/deno/issues/35457.
var __require = /* @__PURE__ */ ((x) => typeof require !== "undefined" ? require : typeof Proxy !== "undefined" ? new Proxy(x, {
  get: (a, b) => (typeof require !== "undefined" ? require : a)[b]
}) : x)(function(x) {
  if (typeof require !== "undefined") return require.apply(this, arguments);
  throw Error('Dynamic require of "' + x + '" is not supported');
});
var loaded = __require(${JSON.stringify(requiredEsm)});
console.log(loaded.default);
var loadedFromUrl = __require(${JSON.stringify(requiredEsmUrl.href)});
console.log(loadedFromUrl.default);
export default loaded;
`,
);

await import(pathToFileURL(configPath).href);
