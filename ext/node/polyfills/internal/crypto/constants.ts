// Copyright 2018-2026 the Deno authors. MIT license.
// Copyright Joyent, Inc. and Node.js contributors. All rights reserved. MIT license.

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file prefer-primordials

(function () {
const { internals } = __bootstrap;

const kHandle = Symbol("kHandle");
const kKeyObject = Symbol("kKeyObject");

internals.kKeyObject = kKeyObject;

return { kHandle, kKeyObject };
})();
