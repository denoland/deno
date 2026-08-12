// Reproduces the error thrown by `ext/napi` when loading `better-sqlite3`,
// which is built against the legacy V8/nan native addon ABI that Deno does not
// support. See denoland/deno#26034.
throw new Error(
  "Cannot load native addon at /home/me/.cache/deno/npm/registry.npmjs.org/" +
    "better-sqlite3/11.3.0/build/Release/better_sqlite3.node: it was built " +
    "against the legacy Node.js native addon API (NODE_MODULE / nan), which " +
    "Deno does not support. Only Node-API (N-API) addons are supported.",
);
