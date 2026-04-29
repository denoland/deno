// Copyright 2018-2026 the Deno authors. MIT license.
// Copyright Joyent, Inc. and Node.js contributors. All rights reserved. MIT license.

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file prefer-primordials

import { notImplemented } from "ext:deno_node/_utils.ts";
import { Buffer } from "node:buffer";
import {
  ERR_CRYPTO_INVALID_DIGEST,
  ERR_INVALID_ARG_TYPE,
  hideStackFrames,
} from "ext:deno_node/internal/errors.ts";
import {
  isAnyArrayBuffer,
  isArrayBufferView,
} from "ext:deno_node/internal/util/types.ts";
import { crypto as constants } from "ext:deno_node/internal_binding/constants.ts";
import {
  validateInt32,
  validateObject,
} from "ext:deno_node/internal/validators.mjs";
import {
  kHandle,
  kKeyObject,
} from "ext:deno_node/internal/crypto/constants.ts";

export type EllipticCurve = {
  name: string;
  ephemeral: boolean;
  privateKeySize: number;
  publicKeySize: number;
  publicKeySizeCompressed: number;
  sharedSecretSize: number;
};

export const ellipticCurves: Array<EllipticCurve> = [
  {
    name: "secp256k1",
    privateKeySize: 32,
    publicKeySize: 65,
    publicKeySizeCompressed: 33,
    sharedSecretSize: 32,
  }, // Weierstrass-class EC used by Bitcoin
  {
    name: "prime256v1",
    privateKeySize: 32,
    publicKeySize: 65,
    publicKeySizeCompressed: 33,
    sharedSecretSize: 32,
  }, // NIST P-256 EC
  {
    name: "secp256r1",
    privateKeySize: 32,
    publicKeySize: 65,
    publicKeySizeCompressed: 33,
    sharedSecretSize: 32,
  }, // NIST P-256 EC (same as above)
  {
    name: "secp384r1",
    privateKeySize: 48,
    publicKeySize: 97,
    publicKeySizeCompressed: 49,
    sharedSecretSize: 48,
  }, // NIST P-384 EC
  {
    name: "secp224r1",
    privateKeySize: 28,
    publicKeySize: 57,
    publicKeySizeCompressed: 29,
    sharedSecretSize: 28,
  }, // NIST P-224 EC
];

// OpenSSL NID values and cipher metadata.
// NID values sourced from OpenSSL's include/openssl/obj_mac.h.
interface CipherInfoResult {
  name: string;
  nid: number;
  blockSize: number;
  ivLength: number;
  keyLength: number;
  mode: string;
}

const cipherInfoTable: CipherInfoResult[] = [
  {
    name: "aes-128-ecb",
    nid: 418,
    blockSize: 16,
    ivLength: 0,
    keyLength: 16,
    mode: "ecb",
  },
  {
    name: "aes-128-cbc",
    nid: 419,
    blockSize: 16,
    ivLength: 16,
    keyLength: 16,
    mode: "cbc",
  },
  {
    name: "aes-192-ecb",
    nid: 422,
    blockSize: 16,
    ivLength: 0,
    keyLength: 24,
    mode: "ecb",
  },
  {
    name: "aes-192-cbc",
    nid: 423,
    blockSize: 16,
    ivLength: 16,
    keyLength: 24,
    mode: "cbc",
  },
  {
    name: "aes-256-ecb",
    nid: 426,
    blockSize: 16,
    ivLength: 0,
    keyLength: 32,
    mode: "ecb",
  },
  {
    name: "aes-256-cbc",
    nid: 427,
    blockSize: 16,
    ivLength: 16,
    keyLength: 32,
    mode: "cbc",
  },
  {
    name: "aes-128-gcm",
    nid: 895,
    blockSize: 1,
    ivLength: 12,
    keyLength: 16,
    mode: "gcm",
  },
  {
    name: "aes-192-gcm",
    nid: 898,
    blockSize: 1,
    ivLength: 12,
    keyLength: 24,
    mode: "gcm",
  },
  {
    name: "aes-256-gcm",
    nid: 901,
    blockSize: 1,
    ivLength: 12,
    keyLength: 32,
    mode: "gcm",
  },
  {
    name: "des-ede3-cbc",
    nid: 44,
    blockSize: 8,
    ivLength: 8,
    keyLength: 24,
    mode: "cbc",
  },
  {
    name: "aes-128-ctr",
    nid: 904,
    blockSize: 1,
    ivLength: 16,
    keyLength: 16,
    mode: "ctr",
  },
  {
    name: "aes-192-ctr",
    nid: 905,
    blockSize: 1,
    ivLength: 16,
    keyLength: 24,
    mode: "ctr",
  },
  {
    name: "aes-256-ctr",
    nid: 906,
    blockSize: 1,
    ivLength: 16,
    keyLength: 32,
    mode: "ctr",
  },
  {
    name: "chacha20-poly1305",
    nid: 1018,
    blockSize: 1,
    ivLength: 12,
    keyLength: 32,
    mode: "",
  },
];

const cipherInfoByName = new Map<string, CipherInfoResult>();
const cipherInfoByNid = new Map<number, CipherInfoResult>();

for (const info of cipherInfoTable) {
  cipherInfoByName.set(info.name, info);
  cipherInfoByNid.set(info.nid, info);
}

// Aliases
cipherInfoByName.set("aes128", cipherInfoByName.get("aes-128-cbc")!);
cipherInfoByName.set("aes192", cipherInfoByName.get("aes-192-cbc")!);
cipherInfoByName.set("aes256", cipherInfoByName.get("aes-256-cbc")!);

// Ciphers actually supported by the runtime (subset of cipherInfoTable).
const supportedCiphers = [
  "aes-128-ecb",
  "aes-128-cbc",
  "aes-192-ecb",
  "aes-256-ecb",
  "aes-256-cbc",
  "aes-128-gcm",
  "aes-256-gcm",
  "aes-128-ctr",
  "aes-192-ctr",
  "aes-256-ctr",
  "des-ede3-cbc",
  "aes128",
  "aes256",
  "chacha20-poly1305",
];

export function getCiphers(): string[] {
  return supportedCiphers;
}

const hashBlockSizes: Record<string, number> = {
  md5: 64,
  rmd160: 64,
  ripemd160: 64,
  sha1: 64,
  sha224: 64,
  sha256: 64,
  sha384: 128,
  sha512: 128,
  "sha512-224": 128,
  "sha512-256": 128,
  "sha3-224": 144,
  "sha3-256": 136,
  "sha3-384": 104,
  "sha3-512": 72,
  blake2b512: 128,
  blake2s256: 64,
};

export function getHashBlockSize(algorithm: string): number {
  const blockSize = hashBlockSizes[algorithm];
  if (blockSize === undefined) {
    throw new ERR_CRYPTO_INVALID_DIGEST(algorithm);
  }
  return blockSize;
}

export function getCipherInfo(
  nameOrNid: string | number,
  options?: { keyLength?: number; ivLength?: number },
) {
  if (typeof nameOrNid !== "string" && typeof nameOrNid !== "number") {
    throw new ERR_INVALID_ARG_TYPE(
      "nameOrNid",
      ["string", "number"],
      nameOrNid,
    );
  }

  if (typeof nameOrNid === "number") {
    validateInt32(nameOrNid, "nameOrNid");
  }

  let keyLength, ivLength;

  if (options !== undefined) {
    validateObject(options, "options");

    ({ keyLength, ivLength } = options);

    if (keyLength !== undefined) {
      validateInt32(keyLength, "options.keyLength");
    }

    if (ivLength !== undefined) {
      validateInt32(ivLength, "options.ivLength");
    }
  }

  const info = typeof nameOrNid === "number"
    ? cipherInfoByNid.get(nameOrNid)
    : cipherInfoByName.get(nameOrNid);

  if (info === undefined) {
    return undefined;
  }

  if (keyLength !== undefined && info.keyLength !== keyLength) {
    return undefined;
  }

  if (ivLength !== undefined && info.ivLength !== ivLength) {
    return undefined;
  }

  return { ...info };
}

let defaultEncoding = "buffer";

export function setDefaultEncoding(val: string) {
  defaultEncoding = val;
}

export function getDefaultEncoding(): string {
  return defaultEncoding;
}

// This is here because many functions accepted binary strings without
// any explicit encoding in older versions of node, and we don't want
// to break them unnecessarily.
export function toBuf(val: string | Buffer, encoding?: string): Buffer {
  if (typeof val === "string") {
    if (encoding === "buffer") {
      encoding = "utf8";
    }

    return Buffer.from(val, encoding);
  }

  return val;
}

export const validateByteSource = hideStackFrames((val, name) => {
  val = toBuf(val);

  if (isAnyArrayBuffer(val) || isArrayBufferView(val)) {
    return;
  }

  throw new ERR_INVALID_ARG_TYPE(
    name,
    ["string", "ArrayBuffer", "TypedArray", "DataView", "Buffer"],
    val,
  );
});

const curveNames = ellipticCurves.map((x) => x.name);
export function getCurves(): readonly string[] {
  return curveNames;
}

export interface SecureHeapUsage {
  total: number;
  min: number;
  used: number;
  utilization: number;
}

export function secureHeapUsed(): SecureHeapUsage {
  notImplemented("crypto.secureHeapUsed");
}

export function setEngine(_engine: string, _flags: typeof constants) {
  notImplemented("crypto.setEngine");
}

export function getOpenSSLSecLevel(): number {
  return 5; // highest sec level, used in tests.
}

const kAesKeyLengths = [128, 192, 256];

export { kAesKeyLengths, kHandle, kKeyObject };

export default {
  getDefaultEncoding,
  setDefaultEncoding,
  getCiphers,
  getCipherInfo,
  getCurves,
  getHashBlockSize,
  getOpenSSLSecLevel,
  secureHeapUsed,
  setEngine,
  validateByteSource,
  toBuf,
  kHandle,
  kKeyObject,
  kAesKeyLengths,
};
