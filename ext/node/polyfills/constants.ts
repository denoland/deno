// Copyright 2018-2026 the Deno authors. MIT license.

// Based on: https://github.com/nodejs/node/blob/0646eda/lib/constants.js

(function () {
const { core, primordials } = __bootstrap;
const { ObjectAssign } = primordials;

const { constants: osConstants } = core.loadExtScript(
  "ext:deno_node/os.ts",
);
const {
  crypto: cryptoConstants,
  fsConstants,
  zlib: zlibConstants,
} = core.loadExtScript("ext:deno_node/internal_binding/constants.ts");

const defaultExport = ObjectAssign(
  {},
  fsConstants,
  osConstants.dlopen,
  osConstants.errno,
  osConstants.signals,
  osConstants.priority,
  cryptoConstants,
  zlibConstants,
);

return {
  default: defaultExport,
  ...defaultExport,
};
})();
