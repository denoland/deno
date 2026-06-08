// Reproduces the error thrown by `ext/napi` when loading any native addon that
// was built against the legacy V8/nan native addon ABI (the `NODE_MODULE`
// macro) instead of Node-API. See denoland/deno#26656.
throw new Error(
  "Cannot load native addon at /home/me/.cache/deno/npm/registry.npmjs.org/" +
    "some-legacy-addon/1.0.0/build/Release/addon.node: it was built against " +
    "the legacy Node.js native addon API (NODE_MODULE / nan), which Deno does " +
    "not support. Only Node-API (N-API) addons are supported.",
);
