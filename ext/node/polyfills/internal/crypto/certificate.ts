// Copyright 2018-2025 the Deno authors. MIT license.
// Copyright Joyent, Inc. and Node.js contributors. All rights reserved. MIT license.

import {
  op_node_cert_export_challenge,
  op_node_cert_export_public_key,
  op_node_verify_spkac,
} from "ext:core/ops";
import { Buffer } from "node:buffer";
import { getArrayBufferOrView } from "ext:deno_node/internal/crypto/keys.ts";

// The functions contained in this file cover the SPKAC format
// (also referred to as Netscape SPKI). A general description of
// the format can be found at https://en.wikipedia.org/wiki/SPKAC

function verifySpkac(spkac, encoding) {
  return op_node_verify_spkac(
    getArrayBufferOrView(spkac, "spkac", encoding),
  );
}

function exportPublicKey(spkac, encoding) {
  const publicKey = op_node_cert_export_public_key(
    getArrayBufferOrView(spkac, "spkac", encoding),
  );
  return publicKey ? Buffer.from(publicKey) : "";
}

function exportChallenge(spkac, encoding) {
  const challenge = op_node_cert_export_challenge(
    getArrayBufferOrView(spkac, "spkac", encoding),
  );
  return challenge ? Buffer.from(challenge) : "";
}

// The legacy implementation of this exposed the Certificate
// object and required that users create an instance before
// calling the member methods. This API pattern has been
// deprecated, however, as the method implementations do not
// rely on any object state.

// For backwards compatibility reasons, this cannot be converted into a
// ES6 Class.
export function Certificate() {
  // deno-lint-ignore prefer-primordials
  if (!(this instanceof Certificate)) {
    return new Certificate();
  }
}

Certificate.prototype.verifySpkac = verifySpkac;
Certificate.prototype.exportPublicKey = exportPublicKey;
Certificate.prototype.exportChallenge = exportChallenge;

Certificate.exportChallenge = exportChallenge;
Certificate.exportPublicKey = exportPublicKey;
Certificate.verifySpkac = verifySpkac;

export default Certificate;
