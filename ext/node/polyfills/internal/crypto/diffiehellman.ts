// Copyright 2018-2026 the Deno authors. MIT license.
// Copyright Joyent, Inc. and Node.js contributors. All rights reserved. MIT license.

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file prefer-primordials

import { core } from "ext:core/mod.js";
import {
  op_node_dh_check,
  op_node_dh_compute_secret,
  op_node_dh_keys_generate_and_export,
  op_node_diffie_hellman,
  op_node_ecdh_compute_public_key,
  op_node_ecdh_compute_secret,
  op_node_ecdh_encode_pubkey,
  op_node_ecdh_generate_keys,
  op_node_ecdh_validate_private_key,
  op_node_ecdh_validate_public_key,
  op_node_gen_prime,
} from "ext:core/ops";

const {
  isAnyArrayBuffer,
  isArrayBufferView,
} = core.loadExtScript("ext:deno_node/internal/util/types.ts");
import {
  ERR_CRYPTO_ECDH_INVALID_FORMAT,
  ERR_CRYPTO_ECDH_INVALID_PUBLIC_KEY,
  ERR_CRYPTO_INCOMPATIBLE_KEY,
  ERR_CRYPTO_UNKNOWN_DH_GROUP,
  ERR_INVALID_ARG_TYPE,
  ERR_INVALID_ARG_VALUE,
  NodeError,
} from "ext:deno_node/internal/errors.ts";
const {
  validateInt32,
  validateString,
} = core.loadExtScript("ext:deno_node/internal/validators.mjs");
import { Buffer } from "node:buffer";
import { deprecate } from "node:util";
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
import {
  getArrayBufferOrView,
  getKeyObjectHandle,
  kConsumePrivate,
  kConsumePublic,
  KeyObject,
} from "ext:deno_node/internal/crypto/keys.ts";
import type { BufferEncoding } from "ext:deno_node/_global.d.ts";

const DH_GENERATOR = 2;

export function DiffieHellman(
  sizeOrKey: number | string | ArrayBufferView,
  keyEncoding?: unknown,
  generator?: unknown,
  genEncoding?: unknown,
) {
  return new DiffieHellmanImpl(
    sizeOrKey,
    keyEncoding,
    generator,
    genEncoding,
  );
}

export class DiffieHellmanImpl {
  verifyError!: number;
  #prime: Buffer;
  #primeLength: number;
  #generator: Buffer;
  #privateKey: Buffer;
  #publicKey: Buffer;
  #publicKeyNeedsUpdate = false;

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
      if (isArrayBufferView(sizeOrKey) || isAnyArrayBuffer(sizeOrKey)) {
        this.#prime = Buffer.from(
          isAnyArrayBuffer(sizeOrKey) ? sizeOrKey : sizeOrKey.buffer,
          isArrayBufferView(sizeOrKey) ? sizeOrKey.byteOffset : 0,
          isArrayBufferView(sizeOrKey)
            ? sizeOrKey.byteLength
            : (sizeOrKey as ArrayBuffer).byteLength,
        );
      } else {
        this.#prime = toBuf(sizeOrKey as string, keyEncoding as string);
      }
    } else {
      // The supplied parameter is our primeLength, generate a suitable prime.
      this.#primeLength = sizeOrKey as number;
      if (this.#primeLength < 2) {
        throw new NodeError(
          "ERR_OSSL_DH_MODULUS_TOO_SMALL",
          "modulus too small",
        );
      }

      this.#prime = Buffer.from(
        op_node_gen_prime(this.#primeLength, false, null, null).buffer,
      );
    }

    if (!generator) {
      generator = DH_GENERATOR;
    }

    if (typeof generator === "number") {
      validateInt32(generator, "generator");
      if (generator <= 0 || generator >= 0x7fffffff) {
        throw new NodeError("ERR_OSSL_DH_BAD_GENERATOR", "bad generator");
      }
      // Store with minimal byte representation, matching Node.js/OpenSSL behavior
      if (generator <= 0xff) {
        this.#generator = Buffer.alloc(1);
        this.#generator.writeUint8(generator);
      } else if (generator <= 0xffff) {
        this.#generator = Buffer.alloc(2);
        this.#generator.writeUint16BE(generator);
      } else {
        this.#generator = Buffer.alloc(4);
        this.#generator.writeUint32BE(generator);
      }
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

    this.verifyError = op_node_dh_check(this.#prime, this.#generator);
  }

  #checkGenerator(): number {
    let generator: number;

    if (this.#generator.length == 0) {
      throw new NodeError("ERR_OSSL_DH_BAD_GENERATOR", "bad generator");
    } else if (this.#generator.length == 1) {
      generator = this.#generator.readUint8();
    } else if (this.#generator.length == 2) {
      generator = this.#generator.readUint16BE();
    } else {
      generator = this.#generator.readUint32BE();
    }

    if (generator != 2 && generator != 5) {
      throw new NodeError("ERR_OSSL_DH_BAD_GENERATOR", "bad generator");
    }

    return generator;
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
    inputEncoding?: BinaryToTextEncoding,
    outputEncoding?: BinaryToTextEncoding,
  ): Buffer | string {
    const buf = getArrayBufferOrView(otherPublicKey, "key", inputEncoding);
    if (buf.length === 0) {
      throw new NodeError(
        "ERR_CRYPTO_INVALID_KEYLEN",
        "Unspecified validation error",
      );
    }

    const sharedSecret = op_node_dh_compute_secret(
      this.#prime,
      this.#privateKey,
      buf,
    );

    // Zero-pad the shared secret to the length of the prime, per RFC 4346
    let secretBuf = Buffer.from(sharedSecret.buffer);
    const primeLen = this.#prime.length;
    if (secretBuf.length < primeLen) {
      const padded = Buffer.alloc(primeLen);
      secretBuf.copy(padded, primeLen - secretBuf.length);
      secretBuf = padded;
    }

    if (outputEncoding == undefined || outputEncoding == "buffer") {
      return secretBuf;
    }

    return secretBuf.toString(outputEncoding);
  }

  generateKeys(): Buffer;
  generateKeys(encoding: BinaryToTextEncoding): string;
  generateKeys(_encoding?: BinaryToTextEncoding): Buffer | string {
    const generator = this.#checkGenerator();

    if (this.#privateKey && this.#publicKey && !this.#publicKeyNeedsUpdate) {
      // Both keys already exist and are up to date, no-op
      return this.#publicKey;
    }

    if (this.#privateKey) {
      // Private key set externally, compute public key from it
      const publicKey = op_node_dh_compute_secret(
        this.#prime,
        this.#privateKey,
        this.#generator,
      );
      this.#publicKey = Buffer.from(publicKey.buffer);
      this.#publicKeyNeedsUpdate = false;
    } else {
      // Generate both keys
      const [privateKey, publicKey] = op_node_dh_keys_generate_and_export(
        this.#prime,
        this.#primeLength ?? 0,
        generator,
      );
      this.#privateKey = Buffer.from(privateKey.buffer);
      this.#publicKey = Buffer.from(publicKey.buffer);
      this.#publicKeyNeedsUpdate = false;
    }

    return this.#publicKey;
  }

  getGenerator(): Buffer;
  getGenerator(encoding: BinaryToTextEncoding): string;
  getGenerator(encoding?: BinaryToTextEncoding): Buffer | string {
    if (encoding !== undefined && encoding != "buffer") {
      return this.#generator.toString(encoding);
    }

    return this.#generator;
  }

  getPrime(): Buffer;
  getPrime(encoding: BinaryToTextEncoding): string;
  getPrime(encoding?: BinaryToTextEncoding): Buffer | string {
    if (encoding !== undefined && encoding != "buffer") {
      return this.#prime.toString(encoding);
    }

    return this.#prime;
  }

  getPrivateKey(): Buffer;
  getPrivateKey(encoding: BinaryToTextEncoding): string;
  getPrivateKey(encoding?: BinaryToTextEncoding): Buffer | string {
    if (encoding !== undefined && encoding != "buffer") {
      return this.#privateKey.toString(encoding);
    }

    return this.#privateKey;
  }

  getPublicKey(): Buffer;
  getPublicKey(encoding: BinaryToTextEncoding): string;
  getPublicKey(encoding?: BinaryToTextEncoding): Buffer | string {
    if (encoding !== undefined && encoding != "buffer") {
      return this.#publicKey.toString(encoding);
    }

    return this.#publicKey;
  }

  setPrivateKey(privateKey: ArrayBufferView): void;
  setPrivateKey(privateKey: string, encoding: BufferEncoding): void;
  setPrivateKey(
    privateKey: ArrayBufferView | string,
    encoding?: BufferEncoding,
  ) {
    if (encoding == undefined || encoding == "buffer") {
      this.#privateKey = Buffer.from(privateKey);
    } else {
      this.#privateKey = Buffer.from(privateKey, encoding);
    }
    // Mark public key as needing regeneration
    this.#publicKeyNeedsUpdate = true;
  }

  setPublicKey(publicKey: ArrayBufferView): void;
  setPublicKey(publicKey: string, encoding: BufferEncoding): void;
  setPublicKey(
    publicKey: ArrayBufferView | string,
    encoding?: BufferEncoding,
  ) {
    if (encoding == undefined || encoding == "buffer") {
      this.#publicKey = Buffer.from(publicKey);
    } else {
      this.#publicKey = Buffer.from(publicKey, encoding);
    }
  }
}

const DH_GROUP_NAMES = [
  "modp1",
  "modp2",
  "modp5",
  "modp14",
  "modp15",
  "modp16",
  "modp17",
  "modp18",
];
const DH_GROUPS = {
  "modp1": {
    // 768-bit prime from RFC 2409
    prime: [
      0xFFFFFFFF,
      0xFFFFFFFF,
      0xC90FDAA2,
      0x2168C234,
      0xC4C6628B,
      0x80DC1CD1,
      0x29024E08,
      0x8A67CC74,
      0x020BBEA6,
      0x3B139B22,
      0x514A0879,
      0x8E3404DD,
      0xEF9519B3,
      0xCD3A431B,
      0x302B0A6D,
      0xF25F1437,
      0x4FE1356D,
      0x6D51C245,
      0xE485B576,
      0x625E7EC6,
      0xF44C42E9,
      0xA63A3620,
      0xFFFFFFFF,
      0xFFFFFFFF,
    ],
    generator: 2,
  },
  "modp2": {
    // 1024-bit prime from RFC 2409
    prime: [
      0xFFFFFFFF,
      0xFFFFFFFF,
      0xC90FDAA2,
      0x2168C234,
      0xC4C6628B,
      0x80DC1CD1,
      0x29024E08,
      0x8A67CC74,
      0x020BBEA6,
      0x3B139B22,
      0x514A0879,
      0x8E3404DD,
      0xEF9519B3,
      0xCD3A431B,
      0x302B0A6D,
      0xF25F1437,
      0x4FE1356D,
      0x6D51C245,
      0xE485B576,
      0x625E7EC6,
      0xF44C42E9,
      0xA637ED6B,
      0x0BFF5CB6,
      0xF406B7ED,
      0xEE386BFB,
      0x5A899FA5,
      0xAE9F2411,
      0x7C4B1FE6,
      0x49286651,
      0xECE65381,
      0xFFFFFFFF,
      0xFFFFFFFF,
    ],
    generator: 2,
  },
  "modp5": {
    prime: [
      0xFFFFFFFF,
      0xFFFFFFFF,
      0xC90FDAA2,
      0x2168C234,
      0xC4C6628B,
      0x80DC1CD1,
      0x29024E08,
      0x8A67CC74,
      0x020BBEA6,
      0x3B139B22,
      0x514A0879,
      0x8E3404DD,
      0xEF9519B3,
      0xCD3A431B,
      0x302B0A6D,
      0xF25F1437,
      0x4FE1356D,
      0x6D51C245,
      0xE485B576,
      0x625E7EC6,
      0xF44C42E9,
      0xA637ED6B,
      0x0BFF5CB6,
      0xF406B7ED,
      0xEE386BFB,
      0x5A899FA5,
      0xAE9F2411,
      0x7C4B1FE6,
      0x49286651,
      0xECE45B3D,
      0xC2007CB8,
      0xA163BF05,
      0x98DA4836,
      0x1C55D39A,
      0x69163FA8,
      0xFD24CF5F,
      0x83655D23,
      0xDCA3AD96,
      0x1C62F356,
      0x208552BB,
      0x9ED52907,
      0x7096966D,
      0x670C354E,
      0x4ABC9804,
      0xF1746C08,
      0xCA237327,
      0xFFFFFFFF,
      0xFFFFFFFF,
    ],
    generator: 2,
  },
  "modp14": {
    prime: [
      0xFFFFFFFF,
      0xFFFFFFFF,
      0xC90FDAA2,
      0x2168C234,
      0xC4C6628B,
      0x80DC1CD1,
      0x29024E08,
      0x8A67CC74,
      0x020BBEA6,
      0x3B139B22,
      0x514A0879,
      0x8E3404DD,
      0xEF9519B3,
      0xCD3A431B,
      0x302B0A6D,
      0xF25F1437,
      0x4FE1356D,
      0x6D51C245,
      0xE485B576,
      0x625E7EC6,
      0xF44C42E9,
      0xA637ED6B,
      0x0BFF5CB6,
      0xF406B7ED,
      0xEE386BFB,
      0x5A899FA5,
      0xAE9F2411,
      0x7C4B1FE6,
      0x49286651,
      0xECE45B3D,
      0xC2007CB8,
      0xA163BF05,
      0x98DA4836,
      0x1C55D39A,
      0x69163FA8,
      0xFD24CF5F,
      0x83655D23,
      0xDCA3AD96,
      0x1C62F356,
      0x208552BB,
      0x9ED52907,
      0x7096966D,
      0x670C354E,
      0x4ABC9804,
      0xF1746C08,
      0xCA18217C,
      0x32905E46,
      0x2E36CE3B,
      0xE39E772C,
      0x180E8603,
      0x9B2783A2,
      0xEC07A28F,
      0xB5C55DF0,
      0x6F4C52C9,
      0xDE2BCBF6,
      0x95581718,
      0x3995497C,
      0xEA956AE5,
      0x15D22618,
      0x98FA0510,
      0x15728E5A,
      0x8AACAA68,
      0xFFFFFFFF,
      0xFFFFFFFF,
    ],
    generator: 2,
  },
  "modp15": {
    prime: [
      0xFFFFFFFF,
      0xFFFFFFFF,
      0xC90FDAA2,
      0x2168C234,
      0xC4C6628B,
      0x80DC1CD1,
      0x29024E08,
      0x8A67CC74,
      0x020BBEA6,
      0x3B139B22,
      0x514A0879,
      0x8E3404DD,
      0xEF9519B3,
      0xCD3A431B,
      0x302B0A6D,
      0xF25F1437,
      0x4FE1356D,
      0x6D51C245,
      0xE485B576,
      0x625E7EC6,
      0xF44C42E9,
      0xA637ED6B,
      0x0BFF5CB6,
      0xF406B7ED,
      0xEE386BFB,
      0x5A899FA5,
      0xAE9F2411,
      0x7C4B1FE6,
      0x49286651,
      0xECE45B3D,
      0xC2007CB8,
      0xA163BF05,
      0x98DA4836,
      0x1C55D39A,
      0x69163FA8,
      0xFD24CF5F,
      0x83655D23,
      0xDCA3AD96,
      0x1C62F356,
      0x208552BB,
      0x9ED52907,
      0x7096966D,
      0x670C354E,
      0x4ABC9804,
      0xF1746C08,
      0xCA18217C,
      0x32905E46,
      0x2E36CE3B,
      0xE39E772C,
      0x180E8603,
      0x9B2783A2,
      0xEC07A28F,
      0xB5C55DF0,
      0x6F4C52C9,
      0xDE2BCBF6,
      0x95581718,
      0x3995497C,
      0xEA956AE5,
      0x15D22618,
      0x98FA0510,
      0x15728E5A,
      0x8AAAC42D,
      0xAD33170D,
      0x04507A33,
      0xA85521AB,
      0xDF1CBA64,
      0xECFB8504,
      0x58DBEF0A,
      0x8AEA7157,
      0x5D060C7D,
      0xB3970F85,
      0xA6E1E4C7,
      0xABF5AE8C,
      0xDB0933D7,
      0x1E8C94E0,
      0x4A25619D,
      0xCEE3D226,
      0x1AD2EE6B,
      0xF12FFA06,
      0xD98A0864,
      0xD8760273,
      0x3EC86A64,
      0x521F2B18,
      0x177B200C,
      0xBBE11757,
      0x7A615D6C,
      0x770988C0,
      0xBAD946E2,
      0x08E24FA0,
      0x74E5AB31,
      0x43DB5BFC,
      0xE0FD108E,
      0x4B82D120,
      0xA93AD2CA,
      0xFFFFFFFF,
      0xFFFFFFFF,
    ],
    generator: 2,
  },
  "modp16": {
    prime: [
      0xFFFFFFFF,
      0xFFFFFFFF,
      0xC90FDAA2,
      0x2168C234,
      0xC4C6628B,
      0x80DC1CD1,
      0x29024E08,
      0x8A67CC74,
      0x020BBEA6,
      0x3B139B22,
      0x514A0879,
      0x8E3404DD,
      0xEF9519B3,
      0xCD3A431B,
      0x302B0A6D,
      0xF25F1437,
      0x4FE1356D,
      0x6D51C245,
      0xE485B576,
      0x625E7EC6,
      0xF44C42E9,
      0xA637ED6B,
      0x0BFF5CB6,
      0xF406B7ED,
      0xEE386BFB,
      0x5A899FA5,
      0xAE9F2411,
      0x7C4B1FE6,
      0x49286651,
      0xECE45B3D,
      0xC2007CB8,
      0xA163BF05,
      0x98DA4836,
      0x1C55D39A,
      0x69163FA8,
      0xFD24CF5F,
      0x83655D23,
      0xDCA3AD96,
      0x1C62F356,
      0x208552BB,
      0x9ED52907,
      0x7096966D,
      0x670C354E,
      0x4ABC9804,
      0xF1746C08,
      0xCA18217C,
      0x32905E46,
      0x2E36CE3B,
      0xE39E772C,
      0x180E8603,
      0x9B2783A2,
      0xEC07A28F,
      0xB5C55DF0,
      0x6F4C52C9,
      0xDE2BCBF6,
      0x95581718,
      0x3995497C,
      0xEA956AE5,
      0x15D22618,
      0x98FA0510,
      0x15728E5A,
      0x8AAAC42D,
      0xAD33170D,
      0x04507A33,
      0xA85521AB,
      0xDF1CBA64,
      0xECFB8504,
      0x58DBEF0A,
      0x8AEA7157,
      0x5D060C7D,
      0xB3970F85,
      0xA6E1E4C7,
      0xABF5AE8C,
      0xDB0933D7,
      0x1E8C94E0,
      0x4A25619D,
      0xCEE3D226,
      0x1AD2EE6B,
      0xF12FFA06,
      0xD98A0864,
      0xD8760273,
      0x3EC86A64,
      0x521F2B18,
      0x177B200C,
      0xBBE11757,
      0x7A615D6C,
      0x770988C0,
      0xBAD946E2,
      0x08E24FA0,
      0x74E5AB31,
      0x43DB5BFC,
      0xE0FD108E,
      0x4B82D120,
      0xA9210801,
      0x1A723C12,
      0xA787E6D7,
      0x88719A10,
      0xBDBA5B26,
      0x99C32718,
      0x6AF4E23C,
      0x1A946834,
      0xB6150BDA,
      0x2583E9CA,
      0x2AD44CE8,
      0xDBBBC2DB,
      0x04DE8EF9,
      0x2E8EFC14,
      0x1FBECAA6,
      0x287C5947,
      0x4E6BC05D,
      0x99B2964F,
      0xA090C3A2,
      0x233BA186,
      0x515BE7ED,
      0x1F612970,
      0xCEE2D7AF,
      0xB81BDD76,
      0x2170481C,
      0xD0069127,
      0xD5B05AA9,
      0x93B4EA98,
      0x8D8FDDC1,
      0x86FFB7DC,
      0x90A6C08F,
      0x4DF435C9,
      0x34063199,
      0xFFFFFFFF,
      0xFFFFFFFF,
    ],
    generator: 2,
  },
  "modp17": {
    prime: [
      0xFFFFFFFF,
      0xFFFFFFFF,
      0xC90FDAA2,
      0x2168C234,
      0xC4C6628B,
      0x80DC1CD1,
      0x29024E08,
      0x8A67CC74,
      0x020BBEA6,
      0x3B139B22,
      0x514A0879,
      0x8E3404DD,
      0xEF9519B3,
      0xCD3A431B,
      0x302B0A6D,
      0xF25F1437,
      0x4FE1356D,
      0x6D51C245,
      0xE485B576,
      0x625E7EC6,
      0xF44C42E9,
      0xA637ED6B,
      0x0BFF5CB6,
      0xF406B7ED,
      0xEE386BFB,
      0x5A899FA5,
      0xAE9F2411,
      0x7C4B1FE6,
      0x49286651,
      0xECE45B3D,
      0xC2007CB8,
      0xA163BF05,
      0x98DA4836,
      0x1C55D39A,
      0x69163FA8,
      0xFD24CF5F,
      0x83655D23,
      0xDCA3AD96,
      0x1C62F356,
      0x208552BB,
      0x9ED52907,
      0x7096966D,
      0x670C354E,
      0x4ABC9804,
      0xF1746C08,
      0xCA18217C,
      0x32905E46,
      0x2E36CE3B,
      0xE39E772C,
      0x180E8603,
      0x9B2783A2,
      0xEC07A28F,
      0xB5C55DF0,
      0x6F4C52C9,
      0xDE2BCBF6,
      0x95581718,
      0x3995497C,
      0xEA956AE5,
      0x15D22618,
      0x98FA0510,
      0x15728E5A,
      0x8AAAC42D,
      0xAD33170D,
      0x04507A33,
      0xA85521AB,
      0xDF1CBA64,
      0xECFB8504,
      0x58DBEF0A,
      0x8AEA7157,
      0x5D060C7D,
      0xB3970F85,
      0xA6E1E4C7,
      0xABF5AE8C,
      0xDB0933D7,
      0x1E8C94E0,
      0x4A25619D,
      0xCEE3D226,
      0x1AD2EE6B,
      0xF12FFA06,
      0xD98A0864,
      0xD8760273,
      0x3EC86A64,
      0x521F2B18,
      0x177B200C,
      0xBBE11757,
      0x7A615D6C,
      0x770988C0,
      0xBAD946E2,
      0x08E24FA0,
      0x74E5AB31,
      0x43DB5BFC,
      0xE0FD108E,
      0x4B82D120,
      0xA9210801,
      0x1A723C12,
      0xA787E6D7,
      0x88719A10,
      0xBDBA5B26,
      0x99C32718,
      0x6AF4E23C,
      0x1A946834,
      0xB6150BDA,
      0x2583E9CA,
      0x2AD44CE8,
      0xDBBBC2DB,
      0x04DE8EF9,
      0x2E8EFC14,
      0x1FBECAA6,
      0x287C5947,
      0x4E6BC05D,
      0x99B2964F,
      0xA090C3A2,
      0x233BA186,
      0x515BE7ED,
      0x1F612970,
      0xCEE2D7AF,
      0xB81BDD76,
      0x2170481C,
      0xD0069127,
      0xD5B05AA9,
      0x93B4EA98,
      0x8D8FDDC1,
      0x86FFB7DC,
      0x90A6C08F,
      0x4DF435C9,
      0x34028492,
      0x36C3FAB4,
      0xD27C7026,
      0xC1D4DCB2,
      0x602646DE,
      0xC9751E76,
      0x3DBA37BD,
      0xF8FF9406,
      0xAD9E530E,
      0xE5DB382F,
      0x413001AE,
      0xB06A53ED,
      0x9027D831,
      0x179727B0,
      0x865A8918,
      0xDA3EDBEB,
      0xCF9B14ED,
      0x44CE6CBA,
      0xCED4BB1B,
      0xDB7F1447,
      0xE6CC254B,
      0x33205151,
      0x2BD7AF42,
      0x6FB8F401,
      0x378CD2BF,
      0x5983CA01,
      0xC64B92EC,
      0xF032EA15,
      0xD1721D03,
      0xF482D7CE,
      0x6E74FEF6,
      0xD55E702F,
      0x46980C82,
      0xB5A84031,
      0x900B1C9E,
      0x59E7C97F,
      0xBEC7E8F3,
      0x23A97A7E,
      0x36CC88BE,
      0x0F1D45B7,
      0xFF585AC5,
      0x4BD407B2,
      0x2B4154AA,
      0xCC8F6D7E,
      0xBF48E1D8,
      0x14CC5ED2,
      0x0F8037E0,
      0xA79715EE,
      0xF29BE328,
      0x06A1D58B,
      0xB7C5DA76,
      0xF550AA3D,
      0x8A1FBFF0,
      0xEB19CCB1,
      0xA313D55C,
      0xDA56C9EC,
      0x2EF29632,
      0x387FE8D7,
      0x6E3C0468,
      0x043E8F66,
      0x3F4860EE,
      0x12BF2D5B,
      0x0B7474D6,
      0xE694F91E,
      0x6DCC4024,
      0xFFFFFFFF,
      0xFFFFFFFF,
    ],
    generator: 2,
  },
  "modp18": {
    prime: [
      0xFFFFFFFF,
      0xFFFFFFFF,
      0xC90FDAA2,
      0x2168C234,
      0xC4C6628B,
      0x80DC1CD1,
      0x29024E08,
      0x8A67CC74,
      0x020BBEA6,
      0x3B139B22,
      0x514A0879,
      0x8E3404DD,
      0xEF9519B3,
      0xCD3A431B,
      0x302B0A6D,
      0xF25F1437,
      0x4FE1356D,
      0x6D51C245,
      0xE485B576,
      0x625E7EC6,
      0xF44C42E9,
      0xA637ED6B,
      0x0BFF5CB6,
      0xF406B7ED,
      0xEE386BFB,
      0x5A899FA5,
      0xAE9F2411,
      0x7C4B1FE6,
      0x49286651,
      0xECE45B3D,
      0xC2007CB8,
      0xA163BF05,
      0x98DA4836,
      0x1C55D39A,
      0x69163FA8,
      0xFD24CF5F,
      0x83655D23,
      0xDCA3AD96,
      0x1C62F356,
      0x208552BB,
      0x9ED52907,
      0x7096966D,
      0x670C354E,
      0x4ABC9804,
      0xF1746C08,
      0xCA18217C,
      0x32905E46,
      0x2E36CE3B,
      0xE39E772C,
      0x180E8603,
      0x9B2783A2,
      0xEC07A28F,
      0xB5C55DF0,
      0x6F4C52C9,
      0xDE2BCBF6,
      0x95581718,
      0x3995497C,
      0xEA956AE5,
      0x15D22618,
      0x98FA0510,
      0x15728E5A,
      0x8AAAC42D,
      0xAD33170D,
      0x04507A33,
      0xA85521AB,
      0xDF1CBA64,
      0xECFB8504,
      0x58DBEF0A,
      0x8AEA7157,
      0x5D060C7D,
      0xB3970F85,
      0xA6E1E4C7,
      0xABF5AE8C,
      0xDB0933D7,
      0x1E8C94E0,
      0x4A25619D,
      0xCEE3D226,
      0x1AD2EE6B,
      0xF12FFA06,
      0xD98A0864,
      0xD8760273,
      0x3EC86A64,
      0x521F2B18,
      0x177B200C,
      0xBBE11757,
      0x7A615D6C,
      0x770988C0,
      0xBAD946E2,
      0x08E24FA0,
      0x74E5AB31,
      0x43DB5BFC,
      0xE0FD108E,
      0x4B82D120,
      0xA9210801,
      0x1A723C12,
      0xA787E6D7,
      0x88719A10,
      0xBDBA5B26,
      0x99C32718,
      0x6AF4E23C,
      0x1A946834,
      0xB6150BDA,
      0x2583E9CA,
      0x2AD44CE8,
      0xDBBBC2DB,
      0x04DE8EF9,
      0x2E8EFC14,
      0x1FBECAA6,
      0x287C5947,
      0x4E6BC05D,
      0x99B2964F,
      0xA090C3A2,
      0x233BA186,
      0x515BE7ED,
      0x1F612970,
      0xCEE2D7AF,
      0xB81BDD76,
      0x2170481C,
      0xD0069127,
      0xD5B05AA9,
      0x93B4EA98,
      0x8D8FDDC1,
      0x86FFB7DC,
      0x90A6C08F,
      0x4DF435C9,
      0x34028492,
      0x36C3FAB4,
      0xD27C7026,
      0xC1D4DCB2,
      0x602646DE,
      0xC9751E76,
      0x3DBA37BD,
      0xF8FF9406,
      0xAD9E530E,
      0xE5DB382F,
      0x413001AE,
      0xB06A53ED,
      0x9027D831,
      0x179727B0,
      0x865A8918,
      0xDA3EDBEB,
      0xCF9B14ED,
      0x44CE6CBA,
      0xCED4BB1B,
      0xDB7F1447,
      0xE6CC254B,
      0x33205151,
      0x2BD7AF42,
      0x6FB8F401,
      0x378CD2BF,
      0x5983CA01,
      0xC64B92EC,
      0xF032EA15,
      0xD1721D03,
      0xF482D7CE,
      0x6E74FEF6,
      0xD55E702F,
      0x46980C82,
      0xB5A84031,
      0x900B1C9E,
      0x59E7C97F,
      0xBEC7E8F3,
      0x23A97A7E,
      0x36CC88BE,
      0x0F1D45B7,
      0xFF585AC5,
      0x4BD407B2,
      0x2B4154AA,
      0xCC8F6D7E,
      0xBF48E1D8,
      0x14CC5ED2,
      0x0F8037E0,
      0xA79715EE,
      0xF29BE328,
      0x06A1D58B,
      0xB7C5DA76,
      0xF550AA3D,
      0x8A1FBFF0,
      0xEB19CCB1,
      0xA313D55C,
      0xDA56C9EC,
      0x2EF29632,
      0x387FE8D7,
      0x6E3C0468,
      0x043E8F66,
      0x3F4860EE,
      0x12BF2D5B,
      0x0B7474D6,
      0xE694F91E,
      0x6DBE1159,
      0x74A3926F,
      0x12FEE5E4,
      0x38777CB6,
      0xA932DF8C,
      0xD8BEC4D0,
      0x73B931BA,
      0x3BC832B6,
      0x8D9DD300,
      0x741FA7BF,
      0x8AFC47ED,
      0x2576F693,
      0x6BA42466,
      0x3AAB639C,
      0x5AE4F568,
      0x3423B474,
      0x2BF1C978,
      0x238F16CB,
      0xE39D652D,
      0xE3FDB8BE,
      0xFC848AD9,
      0x22222E04,
      0xA4037C07,
      0x13EB57A8,
      0x1A23F0C7,
      0x3473FC64,
      0x6CEA306B,
      0x4BCBC886,
      0x2F8385DD,
      0xFA9D4B7F,
      0xA2C087E8,
      0x79683303,
      0xED5BDD3A,
      0x062B3CF5,
      0xB3A278A6,
      0x6D2A13F8,
      0x3F44F82D,
      0xDF310EE0,
      0x74AB6A36,
      0x4597E899,
      0xA0255DC1,
      0x64F31CC5,
      0x0846851D,
      0xF9AB4819,
      0x5DED7EA1,
      0xB1D510BD,
      0x7EE74D73,
      0xFAF36BC3,
      0x1ECFA268,
      0x359046F4,
      0xEB879F92,
      0x4009438B,
      0x481C6CD7,
      0x889A002E,
      0xD5EE382B,
      0xC9190DA6,
      0xFC026E47,
      0x9558E447,
      0x5677E9AA,
      0x9E3050E2,
      0x765694DF,
      0xC81F56E8,
      0x80B96E71,
      0x60C980DD,
      0x98EDD3DF,
      0xFFFFFFFF,
      0xFFFFFFFF,
    ],
    generator: 2,
  },
};

DiffieHellman.prototype = DiffieHellmanImpl.prototype;

export function DiffieHellmanGroup(name: string) {
  return new DiffieHellmanGroupImpl(name);
}

export class DiffieHellmanGroupImpl {
  verifyError!: number;
  #diffiehellman: DiffieHellmanImpl;

  constructor(name: string) {
    if (!DH_GROUP_NAMES.includes(name)) {
      throw new ERR_CRYPTO_UNKNOWN_DH_GROUP();
    }
    const words = DH_GROUPS[name].prime;
    const buf = Buffer.alloc(words.length * 4);
    for (let i = 0; i < words.length; i++) {
      buf.writeUInt32BE(words[i], i * 4);
    }
    this.#diffiehellman = new DiffieHellmanImpl(
      buf,
      DH_GROUPS[name].generator,
    );
    this.verifyError = 0;
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
    inputEncoding?: BinaryToTextEncoding,
    outputEncoding?: BinaryToTextEncoding,
  ): Buffer | string {
    return this.#diffiehellman.computeSecret(
      otherPublicKey,
      inputEncoding,
      outputEncoding,
    );
  }

  generateKeys(): Buffer;
  generateKeys(encoding: BinaryToTextEncoding): string;
  generateKeys(encoding?: BinaryToTextEncoding): Buffer | string {
    return this.#diffiehellman.generateKeys(encoding);
  }

  getGenerator(): Buffer;
  getGenerator(encoding: BinaryToTextEncoding): string;
  getGenerator(encoding?: BinaryToTextEncoding): Buffer | string {
    return this.#diffiehellman.getGenerator(encoding);
  }

  getPrime(): Buffer;
  getPrime(encoding: BinaryToTextEncoding): string;
  getPrime(encoding?: BinaryToTextEncoding): Buffer | string {
    return this.#diffiehellman.getPrime(encoding);
  }

  getPrivateKey(): Buffer;
  getPrivateKey(encoding: BinaryToTextEncoding): string;
  getPrivateKey(encoding?: BinaryToTextEncoding): Buffer | string {
    return this.#diffiehellman.getPrivateKey(encoding);
  }

  getPublicKey(): Buffer;
  getPublicKey(encoding: BinaryToTextEncoding): string;
  getPublicKey(encoding?: BinaryToTextEncoding): Buffer | string {
    return this.#diffiehellman.getPublicKey(encoding);
  }
}

DiffieHellmanGroup.prototype = DiffieHellmanGroupImpl.prototype;
DiffieHellmanGroup.prototype.constructor = DiffieHellmanGroup;

export function ECDH(curve: string) {
  return new ECDHImpl(curve);
}

function validateEcdhFormat(format: ECDHKeyFormat | string): void {
  if (
    format !== "compressed" &&
    format !== "uncompressed" &&
    format !== "hybrid"
  ) {
    throw new ERR_CRYPTO_ECDH_INVALID_FORMAT(String(format));
  }
}

function ecdhEncode(
  buffer: Buffer,
  encoding?: BinaryToTextEncoding | "buffer",
): Buffer | string {
  if (encoding === undefined || encoding === "buffer") {
    return buffer;
  }
  return buffer.toString(encoding);
}

export class ECDHImpl {
  #curve: EllipticCurve; // the selected curve
  #privbuf: Buffer | null = null; // the private key
  #pubbuf: Buffer | null = null; // the public key

  constructor(curve: string) {
    validateString(curve, "curve");

    const c = ellipticCurves.find((x) => x.name == curve);
    if (c == undefined) {
      throw new Error("invalid curve");
    }

    this.#curve = c;
  }

  static convertKey(
    key: BinaryLike,
    curve: string,
    inputEncoding?: BinaryToTextEncoding,
    outputEncoding?: "latin1" | "hex" | "base64" | "base64url",
    format?: "uncompressed" | "compressed" | "hybrid",
  ): Buffer | string {
    validateString(curve, "curve");
    const buf = getArrayBufferOrView(key, "key", inputEncoding);

    let compress: boolean;
    if (format) {
      if (format === "compressed") {
        compress = true;
      } else if (format === "hybrid" || format === "uncompressed") {
        compress = false;
      } else {
        throw new ERR_CRYPTO_ECDH_INVALID_FORMAT(format);
      }
    } else {
      compress = false;
    }

    let result;
    try {
      result = Buffer.from(
        op_node_ecdh_encode_pubkey(curve, buf, compress),
      );
    } catch (e) {
      if (e instanceof TypeError && e.message === "Unsupported curve") {
        throw new TypeError("Invalid EC curve name");
      }
      throw new Error("Failed to convert Buffer to EC_POINT");
    }

    if (format === "hybrid") {
      // Hybrid format: same as uncompressed but first byte is 06 or 07
      // Get compressed form to determine parity
      const compressedBuf = Buffer.from(
        op_node_ecdh_encode_pubkey(curve, buf, true),
      );
      // compressed first byte is 02 (even) or 03 (odd)
      // hybrid first byte is 06 (even) or 07 (odd)
      result[0] = compressedBuf[0] + 4;
    }

    if (outputEncoding && outputEncoding !== "buffer") {
      return result.toString(outputEncoding);
    }
    return result;
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
    inputEncoding?: BinaryToTextEncoding,
    outputEncoding?: BinaryToTextEncoding,
  ): Buffer | string {
    if (this.#privbuf === null) {
      throw new ERR_CRYPTO_ECDH_INVALID_PUBLIC_KEY();
    }

    const otherBuf = typeof otherPublicKey === "string"
      ? Buffer.from(otherPublicKey, inputEncoding)
      : Buffer.from(
        otherPublicKey.buffer,
        otherPublicKey.byteOffset,
        otherPublicKey.byteLength,
      );

    const secretBuf = Buffer.alloc(this.#curve.sharedSecretSize);

    try {
      op_node_ecdh_compute_secret(
        this.#curve.name,
        this.#privbuf,
        this.#pubbuf,
        otherBuf,
        secretBuf,
      );
    } catch (e) {
      // deno-lint-ignore no-explicit-any
      const err = e as any;
      if (err && err.message === "Invalid key pair") {
        throw new Error("Invalid key pair");
      }
      throw new ERR_CRYPTO_ECDH_INVALID_PUBLIC_KEY();
    }

    return ecdhEncode(secretBuf, outputEncoding ?? "buffer");
  }

  generateKeys(): Buffer;
  generateKeys(encoding: BinaryToTextEncoding, format?: ECDHKeyFormat): string;
  generateKeys(
    encoding?: BinaryToTextEncoding,
    format: ECDHKeyFormat = "uncompressed",
  ): Buffer | string {
    validateEcdhFormat(format);
    const pubbuf = Buffer.alloc(
      format == "compressed"
        ? this.#curve.publicKeySizeCompressed
        : this.#curve.publicKeySize,
    );
    const privbuf = Buffer.alloc(this.#curve.privateKeySize);
    op_node_ecdh_generate_keys(
      this.#curve.name,
      pubbuf,
      privbuf,
      format,
    );
    this.#pubbuf = pubbuf;
    this.#privbuf = privbuf;

    if (format === "hybrid") {
      const compressedBuf = Buffer.from(op_node_ecdh_encode_pubkey(
        this.#curve.name,
        pubbuf,
        true,
      ));
      pubbuf[0] = compressedBuf[0] + 4;
    }

    return ecdhEncode(pubbuf, encoding ?? "buffer");
  }

  getPrivateKey(): Buffer;
  getPrivateKey(encoding: BinaryToTextEncoding): string;
  getPrivateKey(encoding?: BinaryToTextEncoding): Buffer | string {
    if (this.#privbuf === null) {
      throw new Error("Failed to get ECDH private key");
    }
    return ecdhEncode(this.#privbuf, encoding ?? "buffer");
  }

  getPublicKey(): Buffer;
  getPublicKey(encoding: BinaryToTextEncoding, format?: ECDHKeyFormat): string;
  getPublicKey(
    encoding?: BinaryToTextEncoding,
    format: ECDHKeyFormat = "uncompressed",
  ): Buffer | string {
    if (this.#pubbuf === null) {
      throw new Error("Failed to get ECDH public key");
    }
    validateEcdhFormat(format);
    const pubbuf = Buffer.from(op_node_ecdh_encode_pubkey(
      this.#curve.name,
      this.#pubbuf,
      format === "compressed",
    ));
    if (format === "hybrid") {
      const compressedBuf = Buffer.from(op_node_ecdh_encode_pubkey(
        this.#curve.name,
        this.#pubbuf,
        true,
      ));
      pubbuf[0] = compressedBuf[0] + 4;
    }
    return ecdhEncode(pubbuf, encoding ?? "buffer");
  }

  setPrivateKey(privateKey: ArrayBufferView): void;
  setPrivateKey(privateKey: string, encoding: BinaryToTextEncoding): void;
  setPrivateKey(
    privateKey: ArrayBufferView | string,
    encoding?: BinaryToTextEncoding,
  ): Buffer | string {
    const privbuf = typeof privateKey === "string"
      ? Buffer.from(privateKey, encoding)
      : Buffer.from(
        privateKey.buffer,
        privateKey.byteOffset,
        privateKey.byteLength,
      );

    if (!op_node_ecdh_validate_private_key(this.#curve.name, privbuf)) {
      throw new Error("Private key is not valid for specified curve");
    }

    const pubbuf = Buffer.alloc(this.#curve.publicKeySize);
    op_node_ecdh_compute_public_key(this.#curve.name, privbuf, pubbuf);

    this.#privbuf = privbuf;
    this.#pubbuf = pubbuf;

    return pubbuf;
  }

  setPublicKey(publicKey: ArrayBufferView): void;
  setPublicKey(publicKey: string, encoding: BinaryToTextEncoding): void;
  setPublicKey(
    publicKey: ArrayBufferView | string,
    encoding?: BinaryToTextEncoding,
  ): void {
    const pubbuf = typeof publicKey === "string"
      ? Buffer.from(publicKey, encoding)
      : Buffer.from(
        publicKey.buffer,
        publicKey.byteOffset,
        publicKey.byteLength,
      );
    if (!op_node_ecdh_validate_public_key(this.#curve.name, pubbuf)) {
      throw new Error("Failed to convert Buffer to EC_POINT");
    }
    this.#pubbuf = pubbuf;
  }
}

ECDH.prototype = ECDHImpl.prototype;
ECDH.convertKey = ECDHImpl.convertKey;
ECDH.prototype.setPublicKey = deprecate(
  ECDHImpl.prototype.setPublicKey,
  "ecdh.setPublicKey() is deprecated.",
  "DEP0031",
);

function statelessDH(
  privateKeyObject: KeyObject,
  publicKeyObject: KeyObject,
): Buffer {
  // getKeyObjectHandle validates key.type and throws ERR_CRYPTO_INVALID_KEY_OBJECT_TYPE
  // for incompatible key kinds (e.g. secret instead of private), so we must
  // call it before doing the asymmetricKeyType cross-check below.
  const privateKey = getKeyObjectHandle(privateKeyObject, kConsumePrivate);
  const publicKey = getKeyObjectHandle(publicKeyObject, kConsumePublic);

  // Check that the asymmetric key types are compatible. EC and DH report
  // mismatching domain parameters from the underlying op when the curves or
  // group parameters differ; everything else (e.g. x25519 vs x448) is a key
  // type mismatch and should surface as ERR_CRYPTO_INCOMPATIBLE_KEY.
  const privType = privateKeyObject.asymmetricKeyType;
  const pubType = publicKeyObject.asymmetricKeyType;
  if (privType !== undefined && pubType !== undefined && privType !== pubType) {
    throw new ERR_CRYPTO_INCOMPATIBLE_KEY(
      "key types for Diffie-Hellman",
      `${privType} and ${pubType}`,
    );
  }

  try {
    const bytes = op_node_diffie_hellman(privateKey, publicKey);
    return Buffer.from(bytes);
  } catch (err) {
    const e = err as Error & { code?: string };
    if (e && typeof e.message === "string") {
      if (e.message.includes("mismatching domain parameters")) {
        e.code = "ERR_OSSL_MISMATCHING_DOMAIN_PARAMETERS";
      } else if (e.message.includes("failed during derivation")) {
        e.code = "ERR_OSSL_FAILED_DURING_DERIVATION";
      }
    }
    throw e;
  }
}

export function diffieHellman(
  options: {
    privateKey: KeyObject;
    publicKey: KeyObject;
  },
  callback?: (err: Error | null, secret?: Buffer) => void,
): Buffer | void {
  if (callback !== undefined && typeof callback !== "function") {
    throw new ERR_INVALID_ARG_TYPE("callback", "function", callback);
  }
  if (
    typeof options !== "object" || options === null || Array.isArray(options)
  ) {
    throw new ERR_INVALID_ARG_TYPE("options", "object", options);
  }
  if (!options.privateKey) {
    throw new ERR_INVALID_ARG_VALUE("options.privateKey", options.privateKey);
  }
  if (!options.publicKey) {
    throw new ERR_INVALID_ARG_VALUE("options.publicKey", options.publicKey);
  }

  if (callback) {
    try {
      const secret = statelessDH(options.privateKey, options.publicKey);
      callback(null, secret);
    } catch (err) {
      callback(err as Error);
    }
    return;
  }

  return statelessDH(options.privateKey, options.publicKey);
}

export default {
  DiffieHellman,
  DiffieHellmanGroup,
  ECDH,
  diffieHellman,
};
