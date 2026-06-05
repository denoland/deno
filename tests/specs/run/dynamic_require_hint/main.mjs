// Minimal reproduction of esbuild's stock `__require` shim, as emitted in
// CJS-style bundles that are then published as ESM without a `createRequire`
// banner (e.g. storybook/internal/core-server). See denoland/deno#28952.
var __require =
  /* @__PURE__ */ ((x) =>
    typeof require !== "undefined"
      ? require
      : typeof Proxy !== "undefined"
      ? new Proxy(x, {
        get: (a, b) => (typeof require !== "undefined" ? require : a)[b],
      })
      : x)(function (x) {
      if (typeof require !== "undefined") return require.apply(this, arguments);
      throw Error('Dynamic require of "' + x + '" is not supported');
    });

var path = __require("path");
console.log(path);
