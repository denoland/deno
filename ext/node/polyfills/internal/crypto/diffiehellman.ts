// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright Joyent, Inc. and Node.js contributors. All rights reserved. MIT license.

import { notImplemented } from "ext:deno_node/_utils.ts";
import {
  isAnyArrayBuffer,
  isArrayBufferView,
} from "ext:deno_node/internal/util/types.ts";
import { ERR_INVALID_ARG_TYPE } from "ext:deno_node/internal/errors.ts";
import {
  validateInt32,
  validateString,
  validateOneOf,
} from "ext:deno_node/internal/validators.mjs";
import { Buffer } from "ext:deno_node/buffer.ts";
import {
  getDefaultEncoding,
  getCurves,
  isCurveEphemeral,
  toBuf,
} from "ext:deno_node/internal/crypto/util.ts";
import type {
  BinaryLike,
  BinaryToTextEncoding,
  ECDHKeyFormat,
} from "ext:deno_node/internal/crypto/types.ts";
import { KeyObject } from "ext:deno_node/internal/crypto/keys.ts";
import type { BufferEncoding } from "ext:deno_node/_global.d.ts";

const { ops } = Deno.core;

const DH_GENERATOR = 2;

export class DiffieHellman {
  verifyError!: number;

  constructor(
    sizeOrKey: unknown,
    keyEncoding?: unknown,
    generator?: unknown,
    genEncoding?: unknown,
  ) {
    if (
      typeof sizeOrKey !== "number" &&
      typeof sizeOrKey !== "string" &&
      !isArrayBufferView(sizeOrKey) &&
      !isAnyArrayBuffer(sizeOrKey)
    ) {
      throw new ERR_INVALID_ARG_TYPE(
        "sizeOrKey",
        ["number", "string", "ArrayBuffer", "Buffer", "TypedArray", "DataView"],
        sizeOrKey,
      );
    }

    if (typeof sizeOrKey === "number") {
      validateInt32(sizeOrKey, "sizeOrKey");
    }

    if (
      keyEncoding &&
      !Buffer.isEncoding(keyEncoding as BinaryToTextEncoding) &&
      keyEncoding !== "buffer"
    ) {
      genEncoding = generator;
      generator = keyEncoding;
      keyEncoding = false;
    }

    const encoding = getDefaultEncoding();
    keyEncoding = keyEncoding || encoding;
    genEncoding = genEncoding || encoding;

    if (typeof sizeOrKey !== "number") {
      sizeOrKey = toBuf(sizeOrKey as string, keyEncoding as string);
    }

    if (!generator) {
      generator = DH_GENERATOR;
    } else if (typeof generator === "number") {
      validateInt32(generator, "generator");
    } else if (typeof generator === "string") {
      generator = toBuf(generator, genEncoding as string);
    } else if (!isArrayBufferView(generator) && !isAnyArrayBuffer(generator)) {
      throw new ERR_INVALID_ARG_TYPE(
        "generator",
        ["number", "string", "ArrayBuffer", "Buffer", "TypedArray", "DataView"],
        generator,
      );
    }

    notImplemented("crypto.DiffieHellman");
  }

  computeSecret(otherPublicKey: ArrayBufferView): Buffer;
  computeSecret(
    otherPublicKey: string,
    inputEncoding: BinaryToTextEncoding,
  ): Buffer;
  computeSecret(
    otherPublicKey: ArrayBufferView,
    outputEncoding: BinaryToTextEncoding,
  ): string;
  computeSecret(
    otherPublicKey: string,
    inputEncoding: BinaryToTextEncoding,
    outputEncoding: BinaryToTextEncoding,
  ): string;
  computeSecret(
    _otherPublicKey: ArrayBufferView | string,
    _inputEncoding?: BinaryToTextEncoding,
    _outputEncoding?: BinaryToTextEncoding,
  ): Buffer | string {
    notImplemented("crypto.DiffieHellman.prototype.computeSecret");
  }

  generateKeys(): Buffer;
  generateKeys(encoding: BinaryToTextEncoding): string;
  generateKeys(_encoding?: BinaryToTextEncoding): Buffer | string {
    notImplemented("crypto.DiffieHellman.prototype.generateKeys");
  }

  getGenerator(): Buffer;
  getGenerator(encoding: BinaryToTextEncoding): string;
  getGenerator(_encoding?: BinaryToTextEncoding): Buffer | string {
    notImplemented("crypto.DiffieHellman.prototype.getGenerator");
  }

  getPrime(): Buffer;
  getPrime(encoding: BinaryToTextEncoding): string;
  getPrime(_encoding?: BinaryToTextEncoding): Buffer | string {
    notImplemented("crypto.DiffieHellman.prototype.getPrime");
  }

  getPrivateKey(): Buffer;
  getPrivateKey(encoding: BinaryToTextEncoding): string;
  getPrivateKey(_encoding?: BinaryToTextEncoding): Buffer | string {
    notImplemented("crypto.DiffieHellman.prototype.getPrivateKey");
  }

  getPublicKey(): Buffer;
  getPublicKey(encoding: BinaryToTextEncoding): string;
  getPublicKey(_encoding?: BinaryToTextEncoding): Buffer | string {
    notImplemented("crypto.DiffieHellman.prototype.getPublicKey");
  }

  setPrivateKey(privateKey: ArrayBufferView): void;
  setPrivateKey(privateKey: string, encoding: BufferEncoding): void;
  setPrivateKey(
    _privateKey: ArrayBufferView | string,
    _encoding?: BufferEncoding,
  ) {
    notImplemented("crypto.DiffieHellman.prototype.setPrivateKey");
  }

  setPublicKey(publicKey: ArrayBufferView): void;
  setPublicKey(publicKey: string, encoding: BufferEncoding): void;
  setPublicKey(
    _publicKey: ArrayBufferView | string,
    _encoding?: BufferEncoding,
  ) {
    notImplemented("crypto.DiffieHellman.prototype.setPublicKey");
  }
}

export class DiffieHellmanGroup {
  verifyError!: number;

  constructor(_name: string) {
    notImplemented("crypto.DiffieHellmanGroup");
  }

  computeSecret(otherPublicKey: ArrayBufferView): Buffer;
  computeSecret(
    otherPublicKey: string,
    inputEncoding: BinaryToTextEncoding,
  ): Buffer;
  computeSecret(
    otherPublicKey: ArrayBufferView,
    outputEncoding: BinaryToTextEncoding,
  ): string;
  computeSecret(
    otherPublicKey: string,
    inputEncoding: BinaryToTextEncoding,
    outputEncoding: BinaryToTextEncoding,
  ): string;
  computeSecret(
    _otherPublicKey: ArrayBufferView | string,
    _inputEncoding?: BinaryToTextEncoding,
    _outputEncoding?: BinaryToTextEncoding,
  ): Buffer | string {
    notImplemented("crypto.DiffieHellman.prototype.computeSecret");
  }

  generateKeys(): Buffer;
  generateKeys(encoding: BinaryToTextEncoding): string;
  generateKeys(_encoding?: BinaryToTextEncoding): Buffer | string {
    notImplemented("crypto.DiffieHellman.prototype.generateKeys");
  }

  getGenerator(): Buffer;
  getGenerator(encoding: BinaryToTextEncoding): string;
  getGenerator(_encoding?: BinaryToTextEncoding): Buffer | string {
    notImplemented("crypto.DiffieHellman.prototype.getGenerator");
  }

  getPrime(): Buffer;
  getPrime(encoding: BinaryToTextEncoding): string;
  getPrime(_encoding?: BinaryToTextEncoding): Buffer | string {
    notImplemented("crypto.DiffieHellman.prototype.getPrime");
  }

  getPrivateKey(): Buffer;
  getPrivateKey(encoding: BinaryToTextEncoding): string;
  getPrivateKey(_encoding?: BinaryToTextEncoding): Buffer | string {
    notImplemented("crypto.DiffieHellman.prototype.getPrivateKey");
  }

  getPublicKey(): Buffer;
  getPublicKey(encoding: BinaryToTextEncoding): string;
  getPublicKey(_encoding?: BinaryToTextEncoding): Buffer | string {
    notImplemented("crypto.DiffieHellman.prototype.getPublicKey");
  }
}

export class ECDH {
  curve: string; // the selected curve
  privbuf: Buffer; // the private key
  pubbuf: Buffer; // the public key
  ephemeral_secret: boolean; // if true the secret is in the resource table
  ephemeral_secret_resource: number; // the resource id in the table

  constructor(curve: string) {
    validateString(curve, "curve");
    validateOneOf(curve, "curve", getCurves());

    this.curve = curve;
    this.ephemeral_secret = isCurveEphemeral(this.curve);
  }

  static convertKey(
    _key: BinaryLike,
    _curve: string,
    _inputEncoding?: BinaryToTextEncoding,
    _outputEncoding?: "latin1" | "hex" | "base64" | "base64url",
    _format?: "uncompressed" | "compressed" | "hybrid",
  ): Buffer | string {
    notImplemented("crypto.ECDH.prototype.convertKey");
  }

  computeSecret(otherPublicKey: ArrayBufferView): Buffer;
  computeSecret(
    otherPublicKey: string,
    inputEncoding: BinaryToTextEncoding,
  ): Buffer;
  computeSecret(
    otherPublicKey: ArrayBufferView,
    outputEncoding: BinaryToTextEncoding,
  ): string;
  computeSecret(
    otherPublicKey: string,
    inputEncoding: BinaryToTextEncoding,
    outputEncoding: BinaryToTextEncoding,
  ): string;
  computeSecret(
    otherPublicKey: ArrayBufferView | string,
    _inputEncoding?: BinaryToTextEncoding,
    _outputEncoding?: BinaryToTextEncoding,
  ): Buffer | string {
    let secretBuf = Buffer.alloc(32);

    if (this.ephemeral_secret) {
      console.log("ephemeral curve: ", this.curve);
      ops.op_node_ecdh_compute_secret(this.curve, this.ephemeral_secret_resource, null, otherPublicKey, secretBuf);
    } else {
      console.log("computeSecret/NOT ephemeral curve:", this.curve);
      console.log(this.privbuf);
      ops.op_node_ecdh_compute_secret(this.curve, 0, this.privbuf, otherPublicKey, secretBuf);
    }

    return secretBuf;
  }

  generateKeys(): Buffer;
  generateKeys(encoding: BinaryToTextEncoding, format?: ECDHKeyFormat): string;
  generateKeys(
    _encoding?: BinaryToTextEncoding,
    _format?: ECDHKeyFormat,
  ): Buffer | string {
    let pubbuf = Buffer.alloc(65);
    let privbuf = Buffer.alloc(32);
    let rid = ops.op_node_ecdh_generate_keys(this.curve, pubbuf, privbuf);

    this.pubbuf = pubbuf;
    if (this.ephemeral_secret) {
      console.log("genKeys/ephemeral curve: ", this.curve);
      this.ephemeral_secret_resource = rid;
    } else {
      console.log("genKeys/NOT ephemeral curve: ", this.curve);
      this.privbuf = privbuf;
    }

    return pubbuf;
  }

  getPrivateKey(): Buffer;
  getPrivateKey(encoding: BinaryToTextEncoding): string;
  getPrivateKey(_encoding?: BinaryToTextEncoding): Buffer | string {
    return this.privbuf;
  }

  getPublicKey(): Buffer;
  getPublicKey(encoding: BinaryToTextEncoding, format?: ECDHKeyFormat): string;
  getPublicKey(
    _encoding?: BinaryToTextEncoding,
    _format?: ECDHKeyFormat,
  ): Buffer | string {
    return this.pubbuf;
  }

  setPrivateKey(privateKey: ArrayBufferView): void;
  setPrivateKey(privateKey: string, encoding: BinaryToTextEncoding): void;
  setPrivateKey(
    privateKey: ArrayBufferView | string,
    _encoding?: BinaryToTextEncoding,
  ): Buffer | string {
    this.privbuf = privateKey;
    let pubbuf = Buffer.alloc(65);

    ops.op_node_ecdh_compute_public_key(this.curve, this.privbuf, pubbuf);
    this.pubbuf = pubbuf;

    return this.pubbuf;
  }
}

export function diffieHellman(_options: {
  privateKey: KeyObject;
  publicKey: KeyObject;
}): Buffer {
  notImplemented("crypto.diffieHellman");
}

export default {
  DiffieHellman,
  DiffieHellmanGroup,
  ECDH,
  diffieHellman,
};
