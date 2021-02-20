// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

// deno-lint-ignore-file camelcase

/// <reference no-default-lib="true" />
/// <reference lib="esnext" />

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
      | Float32Array
      | Float64Array
      | DataView
      | null,
  >(
    array: T,
  ): T;
}

declare interface SubtleCrypto {
  decrypt(
    algorithm:
      | AlgorithmIdentifier
      | RsaOaepParams
      | AesCtrParams
      | AesCbcParams
      | AesCmacParams
      | AesGcmParams
      | AesCfbParams,
    key: CryptoKey,
    data:
      | Int8Array
      | Int16Array
      | Int32Array
      | Uint8Array
      | Uint16Array
      | Uint32Array
      | Uint8ClampedArray
      | Float32Array
      | Float64Array
      | DataView
      | ArrayBuffer,
  ): Promise<ArrayBuffer>;
  deriveBits(
    algorithm:
      | AlgorithmIdentifier
      | EcdhKeyDeriveParams
      | DhKeyDeriveParams
      | ConcatParams
      | HkdfParams
      | Pbkdf2Params,
    baseKey: CryptoKey,
    length: number,
  ): Promise<ArrayBuffer>;
  deriveKey(
    algorithm:
      | AlgorithmIdentifier
      | EcdhKeyDeriveParams
      | DhKeyDeriveParams
      | ConcatParams
      | HkdfParams
      | Pbkdf2Params,
    baseKey: CryptoKey,
    derivedKeyType:
      | string
      | AesDerivedKeyParams
      | HmacImportParams
      | ConcatParams
      | HkdfParams
      | Pbkdf2Params,
    extractable: boolean,
    keyUsages: KeyUsage[],
  ): Promise<CryptoKey>;
  digest(
    algorithm: AlgorithmIdentifier,
    data:
      | Int8Array
      | Int16Array
      | Int32Array
      | Uint8Array
      | Uint16Array
      | Uint32Array
      | Uint8ClampedArray
      | Float32Array
      | Float64Array
      | DataView
      | ArrayBuffer,
  ): Promise<ArrayBuffer>;
  encrypt(
    algorithm:
      | AlgorithmIdentifier
      | RsaOaepParams
      | AesCtrParams
      | AesCbcParams
      | AesCmacParams
      | AesGcmParams
      | AesCfbParams,
    key: CryptoKey,
    data:
      | Int8Array
      | Int16Array
      | Int32Array
      | Uint8Array
      | Uint16Array
      | Uint32Array
      | Uint8ClampedArray
      | Float32Array
      | Float64Array
      | DataView
      | ArrayBuffer,
  ): Promise<ArrayBuffer>;
  exportKey(format: "jwk", key: CryptoKey): Promise<JsonWebKey>;
  exportKey(
    format: "raw" | "pkcs8" | "spki",
    key: CryptoKey,
  ): Promise<ArrayBuffer>;
  exportKey(format: string, key: CryptoKey): Promise<JsonWebKey | ArrayBuffer>;
  generateKey(
    algorithm: RsaHashedKeyGenParams | EcKeyGenParams | DhKeyGenParams,
    extractable: boolean,
    keyUsages: Iterable<KeyUsage>,
  ): Promise<CryptoKeyPair>;
  generateKey(
    algorithm: AesKeyGenParams | HmacKeyGenParams | Pbkdf2Params,
    extractable: boolean,
    keyUsages: Iterable<KeyUsage>,
  ): Promise<CryptoKey>;
  generateKey(
    algorithm: AlgorithmIdentifier,
    extractable: boolean,
    keyUsages: Iterable<KeyUsage>,
  ): Promise<CryptoKeyPair | CryptoKey>;
  importKey(
    format: "jwk",
    keyData: JsonWebKey,
    algorithm:
      | AlgorithmIdentifier
      | RsaHashedImportParams
      | EcKeyImportParams
      | HmacImportParams
      | DhImportKeyParams
      | AesKeyAlgorithm,
    extractable: boolean,
    keyUsages: KeyUsage[],
  ): Promise<CryptoKey>;
  importKey(
    format: "raw" | "pkcs8" | "spki",
    keyData:
      | Int8Array
      | Int16Array
      | Int32Array
      | Uint8Array
      | Uint16Array
      | Uint32Array
      | Uint8ClampedArray
      | Float32Array
      | Float64Array
      | DataView
      | ArrayBuffer,
    algorithm:
      | AlgorithmIdentifier
      | RsaHashedImportParams
      | EcKeyImportParams
      | HmacImportParams
      | DhImportKeyParams
      | AesKeyAlgorithm,
    extractable: boolean,
    keyUsages: KeyUsage[],
  ): Promise<CryptoKey>;
  importKey(
    format: string,
    keyData:
      | JsonWebKey
      | Int8Array
      | Int16Array
      | Int32Array
      | Uint8Array
      | Uint16Array
      | Uint32Array
      | Uint8ClampedArray
      | Float32Array
      | Float64Array
      | DataView
      | ArrayBuffer,
    algorithm:
      | AlgorithmIdentifier
      | RsaHashedImportParams
      | EcKeyImportParams
      | HmacImportParams
      | DhImportKeyParams
      | AesKeyAlgorithm,
    extractable: boolean,
    keyUsages: KeyUsage[],
  ): Promise<CryptoKey>;
  sign(
    algorithm: AlgorithmIdentifier | RsaPssParams | EcdsaParams | AesCmacParams,
    key: CryptoKey,
    data:
      | Int8Array
      | Int16Array
      | Int32Array
      | Uint8Array
      | Uint16Array
      | Uint32Array
      | Uint8ClampedArray
      | Float32Array
      | Float64Array
      | DataView
      | ArrayBuffer,
  ): Promise<ArrayBuffer>;
  unwrapKey(
    format: "raw" | "pkcs8" | "spki" | "jwk" | string,
    wrappedKey:
      | Int8Array
      | Int16Array
      | Int32Array
      | Uint8Array
      | Uint16Array
      | Uint32Array
      | Uint8ClampedArray
      | Float32Array
      | Float64Array
      | DataView
      | ArrayBuffer,
    unwrappingKey: CryptoKey,
    unwrapAlgorithm:
      | AlgorithmIdentifier
      | RsaOaepParams
      | AesCtrParams
      | AesCbcParams
      | AesCmacParams
      | AesGcmParams
      | AesCfbParams,
    unwrappedKeyAlgorithm:
      | AlgorithmIdentifier
      | RsaHashedImportParams
      | EcKeyImportParams
      | HmacImportParams
      | DhImportKeyParams
      | AesKeyAlgorithm,
    extractable: boolean,
    keyUsages: KeyUsage[],
  ): Promise<CryptoKey>;
  verify(
    algorithm: AlgorithmIdentifier | RsaPssParams | EcdsaParams | AesCmacParams,
    key: CryptoKey,
    signature:
      | Int8Array
      | Int16Array
      | Int32Array
      | Uint8Array
      | Uint16Array
      | Uint32Array
      | Uint8ClampedArray
      | Float32Array
      | Float64Array
      | DataView
      | ArrayBuffer,
    data:
      | Int8Array
      | Int16Array
      | Int32Array
      | Uint8Array
      | Uint16Array
      | Uint32Array
      | Uint8ClampedArray
      | Float32Array
      | Float64Array
      | DataView
      | ArrayBuffer,
  ): Promise<boolean>;
  wrapKey(
    format: "raw" | "pkcs8" | "spki" | "jwk" | string,
    key: CryptoKey,
    wrappingKey: CryptoKey,
    wrapAlgorithm:
      | AlgorithmIdentifier
      | RsaOaepParams
      | AesCtrParams
      | AesCbcParams
      | AesCmacParams
      | AesGcmParams
      | AesCfbParams,
  ): Promise<ArrayBuffer>;
}

type AlgorithmIdentifier = string | Algorithm;

type HashAlgorithmIdentifier = AlgorithmIdentifier;

type KeyType = "private" | "public" | "secret";

type KeyUsage =
  | "decrypt"
  | "deriveBits"
  | "deriveKey"
  | "encrypt"
  | "sign"
  | "unwrapKey"
  | "verify"
  | "wrapKey";

interface CryptoKey {
  readonly algorithm: Algorithm;
  readonly extractable: boolean;
  readonly type: KeyType;
  readonly usages: KeyUsage[];
}

interface CryptoKeyPair {
  privateKey?: CryptoKey;
  publicKey?: CryptoKey;
}

interface JsonWebKey {
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

interface Algorithm {
  name: string;
}

interface RsaHashedImportParams extends Algorithm {
  hash: HashAlgorithmIdentifier;
}

interface RsaHashedKeyGenParams extends RsaKeyGenParams {
  hash: HashAlgorithmIdentifier;
}

interface RsaKeyGenParams extends Algorithm {
  modulusLength: number;
  publicExponent: Uint8Array;
}

interface RsaOaepParams extends Algorithm {
  label?:
    | Int8Array
    | Int16Array
    | Int32Array
    | Uint8Array
    | Uint16Array
    | Uint32Array
    | Uint8ClampedArray
    | Float32Array
    | Float64Array
    | DataView
    | ArrayBuffer;
}

interface RsaOtherPrimesInfo {
  d?: string;
  r?: string;
  t?: string;
}

interface RsaPssParams extends Algorithm {
  saltLength: number;
}

interface AesCbcParams extends Algorithm {
  iv:
    | Int8Array
    | Int16Array
    | Int32Array
    | Uint8Array
    | Uint16Array
    | Uint32Array
    | Uint8ClampedArray
    | Float32Array
    | Float64Array
    | DataView
    | ArrayBuffer;
}

interface AesCtrParams extends Algorithm {
  counter:
    | Int8Array
    | Int16Array
    | Int32Array
    | Uint8Array
    | Uint16Array
    | Uint32Array
    | Uint8ClampedArray
    | Float32Array
    | Float64Array
    | DataView
    | ArrayBuffer;
  length: number;
}

interface AesDerivedKeyParams extends Algorithm {
  length: number;
}

interface AesGcmParams extends Algorithm {
  additionalData?:
    | Int8Array
    | Int16Array
    | Int32Array
    | Uint8Array
    | Uint16Array
    | Uint32Array
    | Uint8ClampedArray
    | Float32Array
    | Float64Array
    | DataView
    | ArrayBuffer;
  iv:
    | Int8Array
    | Int16Array
    | Int32Array
    | Uint8Array
    | Uint16Array
    | Uint32Array
    | Uint8ClampedArray
    | Float32Array
    | Float64Array
    | DataView
    | ArrayBuffer;
  tagLength?: number;
}

interface AesKeyAlgorithm extends Algorithm {
  length: number;
}

interface AesKeyGenParams extends Algorithm {
  length: number;
}

interface AesCfbParams extends Algorithm {
  iv:
    | Int8Array
    | Int16Array
    | Int32Array
    | Uint8Array
    | Uint16Array
    | Uint32Array
    | Uint8ClampedArray
    | Float32Array
    | Float64Array
    | DataView
    | ArrayBuffer;
}

interface AesCmacParams extends Algorithm {
  length: number;
}

interface EcKeyGenParams extends Algorithm {
  namedCurve: string;
}

interface EcKeyImportParams extends Algorithm {
  namedCurve: string;
}

interface EcdhKeyDeriveParams extends Algorithm {
  public: CryptoKey;
}

interface EcdsaParams extends Algorithm {
  hash: HashAlgorithmIdentifier;
}

interface DhImportKeyParams extends Algorithm {
  generator: Uint8Array;
  prime: Uint8Array;
}

interface DhKeyAlgorithm extends Algorithm {
  generator: Uint8Array;
  prime: Uint8Array;
}

interface DhKeyDeriveParams extends Algorithm {
  public: CryptoKey;
}

interface DhKeyGenParams extends Algorithm {
  generator: Uint8Array;
  prime: Uint8Array;
}

interface ConcatParams extends Algorithm {
  algorithmId: Uint8Array;
  hash?: string | Algorithm;
  partyUInfo: Uint8Array;
  partyVInfo: Uint8Array;
  privateInfo?: Uint8Array;
  publicInfo?: Uint8Array;
}

interface HkdfParams extends Algorithm {
  hash: HashAlgorithmIdentifier;
  info:
    | Int8Array
    | Int16Array
    | Int32Array
    | Uint8Array
    | Uint16Array
    | Uint32Array
    | Uint8ClampedArray
    | Float32Array
    | Float64Array
    | DataView
    | ArrayBuffer;
  salt:
    | Int8Array
    | Int16Array
    | Int32Array
    | Uint8Array
    | Uint16Array
    | Uint32Array
    | Uint8ClampedArray
    | Float32Array
    | Float64Array
    | DataView
    | ArrayBuffer;
}

interface HmacImportParams extends Algorithm {
  hash: HashAlgorithmIdentifier;
  length?: number;
}

interface HmacKeyGenParams extends Algorithm {
  hash: HashAlgorithmIdentifier;
  length?: number;
}

interface Pbkdf2Params extends Algorithm {
  hash: HashAlgorithmIdentifier;
  iterations: number;
  salt:
    | Int8Array
    | Int16Array
    | Int32Array
    | Uint8Array
    | Uint16Array
    | Uint32Array
    | Uint8ClampedArray
    | Float32Array
    | Float64Array
    | DataView
    | ArrayBuffer;
}
