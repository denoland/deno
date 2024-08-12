// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
// Copyright Joyent, Inc. and Node.js contributors. All rights reserved. MIT license.

import {
  op_node_export_challenge,
  op_node_export_public_key,
  op_node_verify_spkac,
} from "ext:core/ops";
import { Buffer } from "node:buffer";
import { BinaryLike } from "ext:deno_node/internal/crypto/types.ts";

export class Certificate {
  static Certificate = Certificate;
  static exportChallenge(spkac: BinaryLike, encoding?: string): Buffer {
    return Buffer.from(op_node_export_challenge(Buffer.from(spkac, encoding)));
  }

  static exportPublicKey(spkac: BinaryLike, encoding?: string): Buffer {
    return Buffer.from(op_node_export_public_key(Buffer.from(spkac, encoding)));
  }

  static verifySpkac(spkac: BinaryLike, encoding?: string): boolean {
    return op_node_verify_spkac(Buffer.from(spkac, encoding));
  }
}

export default Certificate;
