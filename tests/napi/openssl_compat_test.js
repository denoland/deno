// Copyright 2018-2026 the Deno authors. MIT license.

import { Buffer } from "node:buffer";
import { assert, libSuffix } from "./common.js";
const ops = Deno[Deno.internal].core.ops;
const noop = () => {};

// Legacy Node.js native addons (e.g. NAN-based packages such as
// `nodegit`) reference OpenSSL symbols like `EVP_des_ede3_cbc`
// directly, expecting them to be re-exported by the host binary. Deno
// embeds AWS-LC (a BoringSSL fork) but prefixes the symbols, so they
// are not visible under their conventional names. The compatibility
// shim in `ext/napi/openssl_compat.rs` re-exports a curated set of
// AWS-LC functions under their unprefixed OpenSSL names; this test
// loads a minimal addon that references `EVP_des_ede3_cbc` and
// asserts the load succeeds.
//
// See: https://github.com/denoland/deno/issues/31730
Deno.test("native addon can resolve OpenSSL symbols", {
  ignore: Deno.build.os === "windows",
}, function () {
  const path = new URL(`./module_openssl.${libSuffix}`, import.meta.url)
    .pathname;
  const obj = ops.op_napi_open(
    path,
    {},
    Buffer.from,
    reportError,
    noop,
    noop,
    noop,
    noop,
  );
  assert(obj != null);
  assert(typeof obj === "object");
});
