// Copyright 2018-2025 the Deno authors. MIT license.
// Copyright Joyent, Inc. and Node.js contributors. All rights reserved. MIT license.

import { notImplemented } from "ext:deno_node/_utils.ts";
import { Buffer } from "node:buffer";
import { BinaryLike } from "ext:deno_node/internal/crypto/types.ts";
import { op_node_verify_spkac } from "ext:core/ops";

export class Certificate {
  static Certificate = Certificate;
  static exportChallenge(_spkac: BinaryLike, _encoding?: string): Buffer {
    notImplemented("crypto.Certificate.exportChallenge");
  }

  static exportPublicKey(_spkac: BinaryLike, _encoding?: string): Buffer {
    notImplemented("crypto.Certificate.exportPublicKey");
  }

  static verifySpkac(spkac: BinaryLike, _encoding?: string): boolean {
    return op_node_verify_spkac(spkac);
  }
}

export default Certificate;
