// Copyright 2018-2026 the Deno authors. MIT license.

import { Buffer } from "node:buffer";
import { assert, libSuffix } from "./common.js";
const ops = Deno[Deno.internal].core.ops;
const noop = () => {};

// Legacy Node.js native addons (e.g. NAN-based packages such as
// `nodegit`) reference OpenSSL symbols like `EVP_des_ede3_cbc` directly,
// expecting them to be re-exported by the host binary. Deno does not
// link against the system OpenSSL, so the loader pre-loads
// libcrypto/libssl with `RTLD_GLOBAL` as a compatibility shim. This
// test loads a minimal addon that references `EVP_des_ede3_cbc` and
// asserts the load succeeds.
//
// See: https://github.com/denoland/deno/issues/31730
//
// The shim is Linux/BSD-only. macOS native addons typically link with
// `-undefined dynamic_lookup` and resolve symbols from the host process;
// `dlopen`ing the SIP-protected `libcrypto.dylib` would emit a warning
// and may abort the process. Windows uses a different linkage model.
Deno.test("native addon can resolve OpenSSL symbols", {
  ignore: Deno.build.os === "windows" || Deno.build.os === "darwin",
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
