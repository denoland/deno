// Reproduces the error thrown by `ext/napi` when `isolated-vm`'s native addon
// is built and loaded: it is a legacy V8/nan addon that links against V8's C++
// internals, which Deno does not expose. See denoland/deno#25130.
throw new Error(
  "Cannot load native addon at /home/me/.cache/deno/npm/registry.npmjs.org/" +
    "isolated-vm/4.7.2/out/isolated_vm.node: it was built against the legacy " +
    "Node.js native addon API (NODE_MODULE / nan), which Deno does not " +
    "support. Only Node-API (N-API) addons are supported.",
);
