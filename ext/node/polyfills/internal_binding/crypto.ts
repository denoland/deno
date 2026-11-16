// Copyright 2018-2026 the Deno authors. MIT license.
// Copyright Joyent, Inc. and Node.js contributors. All rights reserved. MIT license.

(function () {
const { core, primordials } = __bootstrap;
const { timingSafeEqual } = core.loadExtScript(
  "ext:deno_node/internal_binding/_timingSafeEqual.ts",
);

const { Error } = primordials;
const { op_node_get_fips } = core.ops;

// Returns 1 when the linked AWS-LC module is in FIPS mode (a `--features
// fips` build), otherwise 0. Matches Node's `crypto.getFips()`.
function getFipsCrypto(): 0 | 1 {
  return op_node_get_fips() === 1 ? 1 : 0;
}

function setFipsCrypto(_fips: boolean) {
  throw new Error("FIPS mode is not supported in Deno.");
}

return { timingSafeEqual, getFipsCrypto, setFipsCrypto };
})();
