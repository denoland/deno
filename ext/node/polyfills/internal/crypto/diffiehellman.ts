// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright Joyent, Inc. and Node.js contributors. All rights reserved. MIT license.

import { notImplemented } from "ext:deno_node/_utils.ts";
import {
  isAnyArrayBuffer,
  isArrayBufferView,
} from "ext:deno_node/internal/util/types.ts";
import { ERR_INVALID_ARG_TYPE, ERR_INVALID_ARG_VALUE} from "ext:deno_node/internal/errors.ts";
import {
  validateInt32,
  validateString,
} from "ext:deno_node/internal/validators.mjs";
import { Buffer } from "ext:deno_node/buffer.ts";
import {
  EllipticCurve,
  ellipticCurves,
  getDefaultEncoding,
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
  #prime: Buffer;
  #primeLength: number;
  #generator: Buffer;
  #privateKey: Buffer;
  #publicKey: Buffer;

  constructor(
    sizeOrKey: number | string | ArrayBufferView,
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
      this.#prime = toBuf(sizeOrKey as string, keyEncoding as string);
    } else {
      // The supplied parameter is our primeLength, generate a suitable prime.
      this.#primeLength = sizeOrKey as number;
      this.#prime = Buffer.from(
        ops.op_node_gen_prime(this.#primeLength).buffer,
      );
    }

    if (!generator) {
      // While the commonly used cyclic group generators for DH are 2 and 5, we
      // need this a buffer, because, well.. Node.
      this.#generator = Buffer.alloc(4);
      this.#generator.writeUint32BE(DH_GENERATOR);
    } else if (typeof generator === "number") {
      validateInt32(generator, "generator");
      this.#generator = Buffer.alloc(4);
      this.#generator.writeUint32BE(generator);
    } else if (typeof generator === "string") {
      generator = toBuf(generator, genEncoding as string);
      this.#generator = generator;
    } else if (!isArrayBufferView(generator) && !isAnyArrayBuffer(generator)) {
      throw new ERR_INVALID_ARG_TYPE(
        "generator",
        ["number", "string", "ArrayBuffer", "Buffer", "TypedArray", "DataView"],
        generator,
      );
    } else {
      this.#generator = Buffer.from(generator);
    }

    this.#checkGenerator();
  }

  #checkGenerator(): void {
    const generator = this.#generator.readUint32BE();

    if (generator != 2 && generator != 5) {
      throw new ERR_INVALID_ARG_VALUE("generator", generator, "should be 2 or 5");
    }
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
    const sharedSecret = ops.op_node_dh_compute_secret(
      this.#prime,
      this.#privateKey,
      Buffer.from(otherPublicKey.buffer),
    );
    return Buffer.from(sharedSecret.buffer);
  }

  generateKeys(): Buffer;
  generateKeys(encoding: BinaryToTextEncoding): string;
  generateKeys(_encoding?: BinaryToTextEncoding): Buffer | string {
    const generator = this.#generator.readUint32BE();
    const [privateKey, publicKey] = ops.op_node_dh_generate2(
      this.#prime,
      this.#primeLength,
      generator,
    );

    this.#privateKey = Buffer.from(privateKey.buffer);
    this.#publicKey = Buffer.from(publicKey.buffer);

    return this.#publicKey;
  }

  getGenerator(): Buffer;
  getGenerator(encoding: BinaryToTextEncoding): string;
  getGenerator(encoding?: BinaryToTextEncoding): Buffer | string {
    if (encoding !== undefined) {
      return this.#generator.toString(encoding);
    }

    return this.#generator;
  }

  getPrime(): Buffer;
  getPrime(encoding: BinaryToTextEncoding): string;
  getPrime(encoding?: BinaryToTextEncoding): Buffer | string {
    if (encoding !== undefined) {
      return this.#prime.toString(encoding);
    }

    return this.#prime;
  }

  getPrivateKey(): Buffer;
  getPrivateKey(encoding: BinaryToTextEncoding): string;
  getPrivateKey(encoding?: BinaryToTextEncoding): Buffer | string {
    if (encoding !== undefined) {
      return this.#privateKey.toString(encoding);
    }

    return this.#privateKey;
  }

  getPublicKey(): Buffer;
  getPublicKey(encoding: BinaryToTextEncoding): string;
  getPublicKey(encoding?: BinaryToTextEncoding): Buffer | string {
    if (encoding !== undefined) {
      return this.#publicKey.toString(encoding);
    }

    return this.#publicKey;
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
  #curve: EllipticCurve; // the selected curve
  #privbuf: Buffer; // the private key
  #pubbuf: Buffer; // the public key

  constructor(curve: string) {
    validateString(curve, "curve");

    const c = ellipticCurves.find((x) => x.name == curve);
    if (c == undefined) {
      throw new Error("invalid curve");
    }

    this.#curve = c;
    this.#pubbuf = Buffer.alloc(this.#curve.publicKeySize);
    this.#privbuf = Buffer.alloc(this.#curve.privateKeySize);
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
    const secretBuf = Buffer.alloc(this.#curve.sharedSecretSize);

    ops.op_node_ecdh_compute_secret(
      this.#curve.name,
      this.#privbuf,
      otherPublicKey,
      secretBuf,
    );

    return secretBuf;
  }

  generateKeys(): Buffer;
  generateKeys(encoding: BinaryToTextEncoding, format?: ECDHKeyFormat): string;
  generateKeys(
    encoding?: BinaryToTextEncoding,
    _format?: ECDHKeyFormat,
  ): Buffer | string {
    ops.op_node_ecdh_generate_keys(
      this.#curve.name,
      this.#pubbuf,
      this.#privbuf,
    );

    if (encoding !== undefined) {
      return this.#pubbuf.toString(encoding);
    }
    return this.#pubbuf;
  }

  getPrivateKey(): Buffer;
  getPrivateKey(encoding: BinaryToTextEncoding): string;
  getPrivateKey(encoding?: BinaryToTextEncoding): Buffer | string {
    if (encoding !== undefined) {
      return this.#privbuf.toString(encoding);
    }
    return this.#privbuf;
  }

  getPublicKey(): Buffer;
  getPublicKey(encoding: BinaryToTextEncoding, format?: ECDHKeyFormat): string;
  getPublicKey(
    encoding?: BinaryToTextEncoding,
    _format?: ECDHKeyFormat,
  ): Buffer | string {
    if (encoding !== undefined) {
      return this.#pubbuf.toString(encoding);
    }
    return this.#pubbuf;
  }

  setPrivateKey(privateKey: ArrayBufferView): void;
  setPrivateKey(privateKey: string, encoding: BinaryToTextEncoding): void;
  setPrivateKey(
    privateKey: ArrayBufferView | string,
    encoding?: BinaryToTextEncoding,
  ): Buffer | string {
    this.#privbuf = privateKey;
    this.#pubbuf = Buffer.alloc(this.#curve.publicKeySize);

    ops.op_node_ecdh_compute_public_key(
      this.#curve.name,
      this.#privbuf,
      this.#pubbuf,
    );

    if (encoding !== undefined) {
      return this.#pubbuf.toString(encoding);
    }
    return this.#pubbuf;
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
