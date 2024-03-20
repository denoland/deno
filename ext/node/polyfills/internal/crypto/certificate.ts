// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
// Copyright Joyent, Inc. and Node.js contributors. All rights reserved. MIT license.

import { notImplemented } from "ext:deno_node/_utils.ts";
import { Buffer } from "node:buffer";
import { BinaryLike } from "ext:deno_node/internal/crypto/types.ts";
import {
  op_node_export_challenge,
  op_node_export_public_key,
  op_node_verify_spkac,
} from "ext:core/ops";

export class Certificate {
  static Certificate = Certificate;
  static exportChallenge(spkac: BinaryLike, encoding?: string): Buffer {
    const buffer = Buffer.from(spkac, encoding);

    const result = op_node_export_challenge(new Uint8Array(buffer.buffer));
    return Buffer.from(result);
  }

  static exportPublicKey(spkac: BinaryLike, encoding?: string): Buffer {
    const buffer = Buffer.from(spkac, encoding);

    const result = op_node_export_public_key(new Uint8Array(buffer.buffer));
    return Buffer.from(result);
  }

  static verifySpkac(spkac: BinaryLike, encoding?: string): boolean {
    const buffer = Buffer.from(spkac, encoding);

    return op_node_verify_spkac(new Uint8Array(buffer.buffer));
  }
}

export default Certificate;
