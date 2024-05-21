// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

// deno-lint-ignore-file no-var

/// <reference no-default-lib="true" />
/// <reference lib="esnext" />

/** @category Crypto */
declare var crypto: Crypto;

/** @category Crypto */
declare interface Algorithm {
  name: string;
}

/** @category Crypto */
declare interface KeyAlgorithm {
  name: string;
}

/** @category Crypto */
declare type AlgorithmIdentifier = string | Algorithm;
/** @category Crypto */
declare type HashAlgorithmIdentifier = AlgorithmIdentifier;
/** @category Crypto */
declare type KeyType = "private" | "public" | "secret";
/** @category Crypto */
declare type KeyUsage =
  | "decrypt"
  | "deriveBits"
  | "deriveKey"
  | "encrypt"
  | "sign"
  | "unwrapKey"
  | "verify"
  | "wrapKey";
/** @category Crypto */
declare type KeyFormat = "jwk" | "pkcs8" | "raw" | "spki";
/** @category Crypto */
declare type NamedCurve = string;

/** @category Crypto */
declare interface RsaOtherPrimesInfo {
  d?: string;
  r?: string;
  t?: string;
}

/** @category Crypto */
declare interface JsonWebKey {
  alg?: string;
  crv?: string;
  d?: string;
  dp?: string;
  dq?: string;
  e?: string;
  ext?: boolean;
  k?: string;
  key_ops?: string[];
  kty?: string;
  n?: string;
  oth?: RsaOtherPrimesInfo[];
  p?: string;
  q?: string;
  qi?: string;
  use?: string;
  x?: string;
  y?: string;
}

/** @category Crypto */
declare interface AesCbcParams extends Algorithm {
  iv: BufferSource;
}

/** @category Crypto */
declare interface AesGcmParams extends Algorithm {
  iv: BufferSource;
  additionalData?: BufferSource;
  tagLength?: number;
}

/** @category Crypto */
declare interface AesCtrParams extends Algorithm {
  counter: BufferSource;
  length: number;
}

/** @category Crypto */
declare interface HmacKeyGenParams extends Algorithm {
  hash: HashAlgorithmIdentifier;
  length?: number;
}

/** @category Crypto */
declare interface EcKeyGenParams extends Algorithm {
  namedCurve: NamedCurve;
}

/** @category Crypto */
declare interface EcKeyImportParams extends Algorithm {
  namedCurve: NamedCurve;
}

/** @category Crypto */
declare interface EcdsaParams extends Algorithm {
  hash: HashAlgorithmIdentifier;
}

/** @category Crypto */
declare interface RsaHashedImportParams extends Algorithm {
  hash: HashAlgorithmIdentifier;
}

/** @category Crypto */
declare interface RsaHashedKeyGenParams extends RsaKeyGenParams {
  hash: HashAlgorithmIdentifier;
}

/** @category Crypto */
declare interface RsaKeyGenParams extends Algorithm {
  modulusLength: number;
  publicExponent: Uint8Array;
}

/** @category Crypto */
declare interface RsaPssParams extends Algorithm {
  saltLength: number;
}

/** @category Crypto */
declare interface RsaOaepParams extends Algorithm {
  label?: Uint8Array;
}

/** @category Crypto */
declare interface HmacImportParams extends Algorithm {
  hash: HashAlgorithmIdentifier;
  length?: number;
}

/** @category Crypto */
declare interface EcKeyAlgorithm extends KeyAlgorithm {
  namedCurve: NamedCurve;
}

/** @category Crypto */
declare interface HmacKeyAlgorithm extends KeyAlgorithm {
  hash: KeyAlgorithm;
  length: number;
}

/** @category Crypto */
declare interface RsaHashedKeyAlgorithm extends RsaKeyAlgorithm {
  hash: KeyAlgorithm;
}

/** @category Crypto */
declare interface RsaKeyAlgorithm extends KeyAlgorithm {
  modulusLength: number;
  publicExponent: Uint8Array;
}

/** @category Crypto */
declare interface HkdfParams extends Algorithm {
  hash: HashAlgorithmIdentifier;
  info: BufferSource;
  salt: BufferSource;
}

/** @category Crypto */
declare interface Pbkdf2Params extends Algorithm {
  hash: HashAlgorithmIdentifier;
  iterations: number;
  salt: BufferSource;
}

/** @category Crypto */
declare interface AesDerivedKeyParams extends Algorithm {
  length: number;
}

/** @category Crypto */
declare interface EcdhKeyDeriveParams extends Algorithm {
  public: CryptoKey;
}

/** @category Crypto */
declare interface AesKeyGenParams extends Algorithm {
  length: number;
}

/** @category Crypto */
declare interface AesKeyAlgorithm extends KeyAlgorithm {
  length: number;
}

/** The CryptoKey dictionary of the Web Crypto API represents a cryptographic
 * key.
 *
 * @category Crypto
 */
declare interface CryptoKey {
  readonly algorithm: KeyAlgorithm;
  readonly extractable: boolean;
  readonly type: KeyType;
  readonly usages: KeyUsage[];
}

/** @category Crypto */
declare var CryptoKey: {
  readonly prototype: CryptoKey;
  new (): never;
};

/** The CryptoKeyPair dictionary of the Web Crypto API represents a key pair for
 * an asymmetric cryptography algorithm, also known as a public-key algorithm.
 *
 * @category Crypto
 */
declare interface CryptoKeyPair {
  privateKey: CryptoKey;
  publicKey: CryptoKey;
}

/** @category Crypto */
declare var CryptoKeyPair: {
  readonly prototype: CryptoKeyPair;
  new (): never;
};

/** This Web Crypto API interface provides a number of low-level cryptographic
 * functions. It is accessed via the Crypto.subtle properties available in a
 * window context (via Window.crypto).
 *
 * @category Crypto
 */
declare interface SubtleCrypto {
  generateKey(
    algorithm: RsaHashedKeyGenParams | EcKeyGenParams,
    extractable: boolean,
    keyUsages: KeyUsage[],
  ): Promise<CryptoKeyPair>;
  generateKey(
    algorithm: AesKeyGenParams | HmacKeyGenParams,
    extractable: boolean,
    keyUsages: KeyUsage[],
  ): Promise<CryptoKey>;
  generateKey(
    algorithm: AlgorithmIdentifier,
    extractable: boolean,
    keyUsages: KeyUsage[],
  ): Promise<CryptoKeyPair | CryptoKey>;
  importKey(
    format: "jwk",
    keyData: JsonWebKey,
    algorithm:
      | AlgorithmIdentifier
      | HmacImportParams
      | RsaHashedImportParams
      | EcKeyImportParams,
    extractable: boolean,
    keyUsages: KeyUsage[],
  ): Promise<CryptoKey>;
  importKey(
    format: Exclude<KeyFormat, "jwk">,
    keyData: BufferSource,
    algorithm:
      | AlgorithmIdentifier
      | HmacImportParams
      | RsaHashedImportParams
      | EcKeyImportParams,
    extractable: boolean,
    keyUsages: KeyUsage[],
  ): Promise<CryptoKey>;
  exportKey(format: "jwk", key: CryptoKey): Promise<JsonWebKey>;
  exportKey(
    format: Exclude<KeyFormat, "jwk">,
    key: CryptoKey,
  ): Promise<ArrayBuffer>;
  sign(
    algorithm: AlgorithmIdentifier | RsaPssParams | EcdsaParams,
    key: CryptoKey,
    data: BufferSource,
  ): Promise<ArrayBuffer>;
  verify(
    algorithm: AlgorithmIdentifier | RsaPssParams | EcdsaParams,
    key: CryptoKey,
    signature: BufferSource,
    data: BufferSource,
  ): Promise<boolean>;
  digest(
    algorithm: AlgorithmIdentifier,
    data: BufferSource,
  ): Promise<ArrayBuffer>;
  encrypt(
    algorithm:
      | AlgorithmIdentifier
      | RsaOaepParams
      | AesCbcParams
      | AesGcmParams
      | AesCtrParams,
    key: CryptoKey,
    data: BufferSource,
  ): Promise<ArrayBuffer>;
  decrypt(
    algorithm:
      | AlgorithmIdentifier
      | RsaOaepParams
      | AesCbcParams
      | AesGcmParams
      | AesCtrParams,
    key: CryptoKey,
    data: BufferSource,
  ): Promise<ArrayBuffer>;
  deriveBits(
    algorithm:
      | AlgorithmIdentifier
      | HkdfParams
      | Pbkdf2Params
      | EcdhKeyDeriveParams,
    baseKey: CryptoKey,
    length: number,
  ): Promise<ArrayBuffer>;
  deriveKey(
    algorithm:
      | AlgorithmIdentifier
      | HkdfParams
      | Pbkdf2Params
      | EcdhKeyDeriveParams,
    baseKey: CryptoKey,
    derivedKeyType:
      | AlgorithmIdentifier
      | AesDerivedKeyParams
      | HmacImportParams
      | HkdfParams
      | Pbkdf2Params,
    extractable: boolean,
    keyUsages: KeyUsage[],
  ): Promise<CryptoKey>;
  wrapKey(
    format: KeyFormat,
    key: CryptoKey,
    wrappingKey: CryptoKey,
    wrapAlgorithm:
      | AlgorithmIdentifier
      | RsaOaepParams
      | AesCbcParams
      | AesCtrParams,
  ): Promise<ArrayBuffer>;
  unwrapKey(
    format: KeyFormat,
    wrappedKey: BufferSource,
    unwrappingKey: CryptoKey,
    unwrapAlgorithm:
      | AlgorithmIdentifier
      | RsaOaepParams
      | AesCbcParams
      | AesCtrParams,
    unwrappedKeyAlgorithm:
      | AlgorithmIdentifier
      | HmacImportParams
      | RsaHashedImportParams
      | EcKeyImportParams,
    extractable: boolean,
    keyUsages: KeyUsage[],
  ): Promise<CryptoKey>;
}

/** @category Crypto */
declare var SubtleCrypto: {
  readonly prototype: SubtleCrypto;
  new (): never;
};

/** @category Crypto */
declare interface Crypto {
  readonly subtle: SubtleCrypto;
  getRandomValues<
    T extends
      | Int8Array
      | Int16Array
      | Int32Array
      | Uint8Array
      | Uint16Array
      | Uint32Array
      | Uint8ClampedArray
      | BigInt64Array
      | BigUint64Array,
  >(
    array: T,
  ): T;
  randomUUID(): `${string}-${string}-${string}-${string}-${string}`;
}

/** @category Crypto */
declare var Crypto: {
  readonly prototype: Crypto;
  new (): never;
};
