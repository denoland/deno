// Copyright 2018-2026 the Deno authors. MIT license.
// Copyright Joyent, Inc. and Node.js contributors. All rights reserved. MIT license.

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file prefer-primordials no-explicit-any

(function () {
const { core } = globalThis.__bootstrap;
const {
  op_node_check_prime_bytes,
  op_node_check_prime_bytes_async,
  op_node_gen_prime,
  op_node_gen_prime_async,
} = core.ops;

const { default: randomBytes } = core.loadExtScript(
  "ext:deno_node/internal/crypto/_randomBytes.ts",
);
const { default: randomFill, randomFillSync } = core.loadExtScript(
  "ext:deno_node/internal/crypto/_randomFill.mjs",
);
const { default: randomInt } = core.loadExtScript(
  "ext:deno_node/internal/crypto/_randomInt.ts",
);
const {
  validateBoolean,
  validateFunction,
  validateInt32,
  validateObject,
} = core.loadExtScript("ext:deno_node/internal/validators.mjs");
const {
  isAnyArrayBuffer,
  isArrayBufferView,
} = core.loadExtScript("ext:deno_node/internal/util/types.ts");
const {
  ERR_INVALID_ARG_TYPE,
  ERR_OUT_OF_RANGE,
  NodeError,
  NodeRangeError,
} = core.loadExtScript("ext:deno_node/internal/errors.ts");
const { Buffer } = core.loadExtScript("ext:deno_node/internal/buffer.mjs");

// OpenSSL BIGNUM max size: INT_MAX / (4 * BN_BITS2) words * 8 bytes/word
// On 64-bit: (2^31 - 1) / 256 * 8 = 67108856 bytes
const OPENSSL_BIGNUM_MAX_BYTES = (((2 ** 31 - 1) / (4 * 64)) | 0) * 8;

function checkPrime(
  candidate: any,
  options: any = {},
  callback?: (err: Error | null, result: boolean) => void,
) {
  if (typeof options === "function") {
    callback = options;
    options = {};
  }

  validateFunction(callback, "callback");
  validateObject(options, "options");

  const {
    checks = 0,
  } = options!;

  validateInt32(checks, "options.checks", 0);

  let candidateBytes: ArrayBufferView | ArrayBuffer;
  if (typeof candidate === "bigint") {
    if (candidate < 0) {
      throw new ERR_OUT_OF_RANGE("candidate", ">= 0", candidate);
    }
    candidateBytes = bigintToBytes(candidate);
  } else if (isAnyArrayBuffer(candidate) || isArrayBufferView(candidate)) {
    const byteLength = isArrayBufferView(candidate)
      ? (candidate as ArrayBufferView).byteLength
      : (candidate as ArrayBuffer).byteLength;
    if (byteLength > OPENSSL_BIGNUM_MAX_BYTES) {
      throw new NodeError(
        "ERR_OSSL_BN_BIGNUM_TOO_LONG",
        "bignum too long",
      );
    }
    candidateBytes = candidate;
  } else {
    throw new ERR_INVALID_ARG_TYPE(
      "candidate",
      [
        "ArrayBuffer",
        "TypedArray",
        "Buffer",
        "DataView",
        "bigint",
      ],
      candidate,
    );
  }

  op_node_check_prime_bytes_async(candidateBytes, checks).then(
    (result) => {
      callback?.(null, result);
    },
  ).catch((err) => {
    callback?.(err, false);
  });
}

function checkPrimeSync(
  candidate: any,
  options: any = {},
): boolean {
  validateObject(options, "options");

  const {
    checks = 0,
  } = options!;

  validateInt32(checks, "options.checks", 0);

  let candidateBytes: ArrayBufferView | ArrayBuffer;
  if (typeof candidate === "bigint") {
    if (candidate < 0) {
      throw new ERR_OUT_OF_RANGE("candidate", ">= 0", candidate);
    }
    candidateBytes = bigintToBytes(candidate);
  } else if (isAnyArrayBuffer(candidate) || isArrayBufferView(candidate)) {
    const byteLength = isArrayBufferView(candidate)
      ? (candidate as ArrayBufferView).byteLength
      : (candidate as ArrayBuffer).byteLength;
    if (byteLength > OPENSSL_BIGNUM_MAX_BYTES) {
      throw new NodeError(
        "ERR_OSSL_BN_BIGNUM_TOO_LONG",
        "bignum too long",
      );
    }
    candidateBytes = candidate;
  } else {
    throw new ERR_INVALID_ARG_TYPE(
      "candidate",
      [
        "ArrayBuffer",
        "TypedArray",
        "Buffer",
        "DataView",
        "bigint",
      ],
      candidate,
    );
  }

  return op_node_check_prime_bytes(candidateBytes, checks);
}

function generatePrime(
  size: number,
  options: any = {},
  callback?: (err: Error | null, prime: ArrayBuffer | bigint) => void,
) {
  validateInt32(size, "size", 1);
  if (typeof options === "function") {
    callback = options;
    options = {};
  }
  validateFunction(callback, "callback");
  const {
    bigint,
    safe,
    add,
    rem,
  } = validateRandomPrimeJob(size, options);
  op_node_gen_prime_async(size, safe, add ?? null, rem ?? null).then(
    (prime: Uint8Array) => {
      const result = bigint
        ? arrayBufferToUnsignedBigInt(prime.buffer)
        : prime.buffer;
      callback?.(null, result);
    },
    (err: Error) => {
      callback?.(err, null as unknown as ArrayBuffer);
    },
  );
}

function generatePrimeSync(
  size: number,
  options: any = {},
): ArrayBuffer | bigint {
  const {
    bigint,
    safe,
    add,
    rem,
  } = validateRandomPrimeJob(size, options);

  const prime = op_node_gen_prime(size, safe, add ?? null, rem ?? null);
  if (bigint) return arrayBufferToUnsignedBigInt(prime.buffer);
  return prime.buffer;
}

function toUint8Array(
  value: ArrayBuffer | ArrayBufferView | Buffer,
): Uint8Array {
  if (value instanceof Uint8Array) {
    return value;
  }
  if (isArrayBufferView(value)) {
    return new Uint8Array(value.buffer, value.byteOffset, value.byteLength);
  }
  return new Uint8Array(value);
}

function validateRandomPrimeJob(
  size: number,
  options: any,
): any {
  validateInt32(size, "size", 1);
  validateObject(options, "options");

  let {
    safe = false,
    bigint = false,
    add,
    rem,
  } = options!;

  validateBoolean(safe, "options.safe");
  validateBoolean(bigint, "options.bigint");

  if (add !== undefined) {
    if (typeof add === "bigint") {
      add = unsignedBigIntToBuffer(add, "options.add");
    } else if (!isAnyArrayBuffer(add) && !isArrayBufferView(add)) {
      throw new ERR_INVALID_ARG_TYPE(
        "options.add",
        [
          "ArrayBuffer",
          "TypedArray",
          "Buffer",
          "DataView",
          "bigint",
        ],
        add,
      );
    }
  }

  if (rem !== undefined) {
    if (typeof rem === "bigint") {
      rem = unsignedBigIntToBuffer(rem, "options.rem");
    } else if (!isAnyArrayBuffer(rem) && !isArrayBufferView(rem)) {
      throw new ERR_INVALID_ARG_TYPE(
        "options.rem",
        [
          "ArrayBuffer",
          "TypedArray",
          "Buffer",
          "DataView",
          "bigint",
        ],
        rem,
      );
    }
  }

  const addBuf = add ? toUint8Array(add) : undefined;
  const remBuf = rem ? toUint8Array(rem) : undefined;

  if (addBuf) {
    const addBitCount = bitCount(addBuf);
    if (addBitCount === 0) {
      throw new NodeRangeError("ERR_OUT_OF_RANGE", "invalid options.add");
    }
    if (addBitCount > size) {
      throw new NodeRangeError("ERR_OUT_OF_RANGE", "invalid options.add");
    }

    if (remBuf) {
      const addBigInt = bufferToBigInt(addBuf);
      const remBigInt = bufferToBigInt(remBuf);
      if (addBigInt <= remBigInt) {
        throw new NodeRangeError("ERR_OUT_OF_RANGE", "invalid options.rem");
      }
    }
  }

  return {
    safe,
    bigint,
    add: addBuf,
    rem: remBuf,
  };
}

function bigintToBytes(n: bigint): Uint8Array {
  if (n === 0n) return new Uint8Array([0]);
  const hex = n.toString(16);
  const padded = hex.length % 2 ? "0" + hex : hex;
  const bytes = new Uint8Array(padded.length / 2);
  for (let i = 0; i < bytes.length; i++) {
    bytes[i] = parseInt(padded.substring(i * 2, i * 2 + 2), 16);
  }
  return bytes;
}

function bufferToBigInt(buf: Uint8Array): bigint {
  let result = 0n;
  for (let i = 0; i < buf.length; i++) {
    result = (result << 8n) | BigInt(buf[i]);
  }
  return result;
}

function bitCount(buf: Uint8Array): number {
  for (let i = 0; i < buf.length; i++) {
    if (buf[i] !== 0) {
      return (buf.length - i) * 8 - Math.clz32(buf[i]) + 24;
    }
  }
  return 0;
}

const numberToHexCharCode = (number: number): number =>
  (number < 10 ? 48 : 87) + number;

function arrayBufferToUnsignedBigInt(buf: ArrayBuffer): bigint {
  const length = buf.byteLength;
  const chars: number[] = Array(length * 2);
  const view = new DataView(buf);

  for (let i = 0; i < length; i++) {
    const val = view.getUint8(i);
    chars[2 * i] = numberToHexCharCode(val >> 4);
    chars[2 * i + 1] = numberToHexCharCode(val & 0xf);
  }

  return BigInt(`0x${String.fromCharCode(...chars)}`);
}

function unsignedBigIntToBuffer(bigint: bigint, name: string) {
  if (bigint < 0) {
    throw new ERR_OUT_OF_RANGE(name, ">= 0", bigint);
  }

  const hex = bigint.toString(16);
  const padded = hex.padStart(hex.length + (hex.length % 2), "0");
  return Buffer.from(padded, "hex");
}

function randomUUID(options) {
  if (options !== undefined) {
    validateObject(options, "options");
  }
  const {
    disableEntropyCache = false,
  } = options || {};

  validateBoolean(disableEntropyCache, "options.disableEntropyCache");

  return globalThis.crypto.randomUUID();
}

return {
  checkPrime,
  checkPrimeSync,
  generatePrime,
  generatePrimeSync,
  randomUUID,
  randomInt,
  randomBytes,
  randomFill,
  randomFillSync,
  default: {
    checkPrime,
    checkPrimeSync,
    generatePrime,
    generatePrimeSync,
    randomUUID,
    randomInt,
    randomBytes,
    randomFill,
    randomFillSync,
  },
};
})();
