import { createHash } from "../../hash/mod.ts";
import { Buffer } from "../buffer.ts";
import { MAX_ALLOC } from "./constants.ts";
import { HASH_DATA } from "./types.ts";

export type NormalizedAlgorithms =
  | "md5"
  | "ripemd160"
  | "sha1"
  | "sha224"
  | "sha256"
  | "sha384"
  | "sha512";

type Algorithms =
  | "md5"
  | "ripemd160"
  | "rmd160"
  | "sha1"
  | "sha224"
  | "sha256"
  | "sha384"
  | "sha512";

function createHasher(alg: Algorithms) {
  let normalizedAlg: NormalizedAlgorithms;
  if (alg === "rmd160") {
    normalizedAlg = "ripemd160";
  } else {
    normalizedAlg = alg;
  }
  return (value: Uint8Array) =>
    Buffer.from(createHash(normalizedAlg).update(value).digest());
}

function getZeroes(zeros: number) {
  return Buffer.alloc(zeros);
}
const sizes = {
  md5: 16,
  sha1: 20,
  sha224: 28,
  sha256: 32,
  sha384: 48,
  sha512: 64,
  rmd160: 20,
  ripemd160: 20,
};

function toBuffer(bufferable: HASH_DATA) {
  if (bufferable instanceof Uint8Array || typeof bufferable === "string") {
    return Buffer.from(bufferable as Uint8Array);
  } else {
    return Buffer.from(bufferable.buffer);
  }
}

class Hmac {
  hash: (value: Uint8Array) => Buffer;
  ipad1: Buffer;
  opad: Buffer;
  alg: string;
  blocksize: number;
  size: number;
  ipad2: Buffer;

  constructor(alg: Algorithms, key: Buffer, saltLen: number) {
    this.hash = createHasher(alg);

    const blocksize = (alg === "sha512" || alg === "sha384") ? 128 : 64;

    if (key.length > blocksize) {
      key = this.hash(key);
    } else if (key.length < blocksize) {
      key = Buffer.concat([key, getZeroes(blocksize - key.length)], blocksize);
    }

    const ipad = Buffer.allocUnsafe(blocksize + sizes[alg]);
    const opad = Buffer.allocUnsafe(blocksize + sizes[alg]);
    for (let i = 0; i < blocksize; i++) {
      ipad[i] = key[i] ^ 0x36;
      opad[i] = key[i] ^ 0x5C;
    }

    const ipad1 = Buffer.allocUnsafe(blocksize + saltLen + 4);
    ipad.copy(ipad1, 0, 0, blocksize);

    this.ipad1 = ipad1;
    this.ipad2 = ipad;
    this.opad = opad;
    this.alg = alg;
    this.blocksize = blocksize;
    this.size = sizes[alg];
  }

  run(data: Buffer, ipad: Buffer) {
    data.copy(ipad, this.blocksize);
    const h = this.hash(ipad);
    h.copy(this.opad, this.blocksize);
    return this.hash(this.opad);
  }
}

/**
 * @param iterations Needs to be higher or equal than zero
 * @param keylen  Needs to be higher or equal than zero but less than max allocation size (2^30)
 * @param digest Algorithm to be used for encryption
 */
export function pbkdf2Sync(
  password: HASH_DATA,
  salt: HASH_DATA,
  iterations: number,
  keylen: number,
  digest: Algorithms = "sha1",
): Buffer {
  if (typeof iterations !== "number" || iterations < 0) {
    throw new TypeError("Bad iterations");
  }
  if (typeof keylen !== "number" || keylen < 0 || keylen > MAX_ALLOC) {
    throw new TypeError("Bad key length");
  }

  const bufferedPassword = toBuffer(password);
  const bufferedSalt = toBuffer(salt);

  const hmac = new Hmac(digest, bufferedPassword, bufferedSalt.length);

  const DK = Buffer.allocUnsafe(keylen);
  const block1 = Buffer.allocUnsafe(bufferedSalt.length + 4);
  bufferedSalt.copy(block1, 0, 0, bufferedSalt.length);

  let destPos = 0;
  const hLen = sizes[digest];
  const l = Math.ceil(keylen / hLen);

  for (let i = 1; i <= l; i++) {
    block1.writeUInt32BE(i, bufferedSalt.length);

    const T = hmac.run(block1, hmac.ipad1);
    let U = T;

    for (let j = 1; j < iterations; j++) {
      U = hmac.run(U, hmac.ipad2);
      for (let k = 0; k < hLen; k++) T[k] ^= U[k];
    }

    T.copy(DK, destPos);
    destPos += hLen;
  }

  return DK;
}

/**
 * @param iterations Needs to be higher or equal than zero
 * @param keylen  Needs to be higher or equal than zero but less than max allocation size (2^30)
 * @param digest Algorithm to be used for encryption
 */
export function pbkdf2(
  password: HASH_DATA,
  salt: HASH_DATA,
  iterations: number,
  keylen: number,
  digest: Algorithms = "sha1",
  callback: ((err?: Error, derivedKey?: Buffer) => void),
): void {
  let err, res;
  try {
    res = pbkdf2Sync(
      password,
      salt,
      iterations,
      keylen,
      digest,
    );
  } catch (e) {
    err = e;
  }
  callback(err, res);
}
