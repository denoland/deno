// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright Joyent, Inc. and Node.js contributors. All rights reserved. MIT license.

import { KeyObject } from "ext:deno_node/internal/crypto/keys.ts";
import { notImplemented } from "ext:deno_node/_utils.ts";
import { Buffer } from "ext:deno_node/buffer.ts";
import { KeyFormat, KeyType } from "ext:deno_node/internal/crypto/types.ts";

export function generateKey(
  _type: "hmac" | "aes",
  _options: {
    length: number;
  },
  _callback: (err: Error | null, key: KeyObject) => void,
) {
  notImplemented("crypto.generateKey");
}

export interface BasePrivateKeyEncodingOptions<T extends KeyFormat> {
  format: T;
  cipher?: string | undefined;
  passphrase?: string | undefined;
}

export interface RSAKeyPairOptions<
  PubF extends KeyFormat,
  PrivF extends KeyFormat,
> {
  /**
   * Key size in bits
   */
  modulusLength: number;
  /**
   * Public exponent
   * @default 0x10001
   */
  publicExponent?: number | undefined;
  publicKeyEncoding: {
    type: "pkcs1" | "spki";
    format: PubF;
  };
  privateKeyEncoding: BasePrivateKeyEncodingOptions<PrivF> & {
    type: "pkcs1" | "pkcs8";
  };
}

export interface RSAPSSKeyPairOptions<
  PubF extends KeyFormat,
  PrivF extends KeyFormat,
> {
  /**
   * Key size in bits
   */
  modulusLength: number;
  /**
   * Public exponent
   * @default 0x10001
   */
  publicExponent?: number | undefined;
  /**
   * Name of the message digest
   */
  hashAlgorithm?: string;
  /**
   * Name of the message digest used by MGF1
   */
  mgf1HashAlgorithm?: string;
  /**
   * Minimal salt length in bytes
   */
  saltLength?: string;
  publicKeyEncoding: {
    type: "spki";
    format: PubF;
  };
  privateKeyEncoding: BasePrivateKeyEncodingOptions<PrivF> & {
    type: "pkcs8";
  };
}

export interface DSAKeyPairOptions<
  PubF extends KeyFormat,
  PrivF extends KeyFormat,
> {
  /**
   * Key size in bits
   */
  modulusLength: number;
  /**
   * Size of q in bits
   */
  divisorLength: number;
  publicKeyEncoding: {
    type: "spki";
    format: PubF;
  };
  privateKeyEncoding: BasePrivateKeyEncodingOptions<PrivF> & {
    type: "pkcs8";
  };
}

export interface ECKeyPairOptions<
  PubF extends KeyFormat,
  PrivF extends KeyFormat,
> {
  /**
   * Name of the curve to use.
   */
  namedCurve: string;
  publicKeyEncoding: {
    type: "pkcs1" | "spki";
    format: PubF;
  };
  privateKeyEncoding: BasePrivateKeyEncodingOptions<PrivF> & {
    type: "sec1" | "pkcs8";
  };
}

export interface ED25519KeyPairOptions<
  PubF extends KeyFormat,
  PrivF extends KeyFormat,
> {
  publicKeyEncoding: {
    type: "spki";
    format: PubF;
  };
  privateKeyEncoding: BasePrivateKeyEncodingOptions<PrivF> & {
    type: "pkcs8";
  };
}

export interface ED448KeyPairOptions<
  PubF extends KeyFormat,
  PrivF extends KeyFormat,
> {
  publicKeyEncoding: {
    type: "spki";
    format: PubF;
  };
  privateKeyEncoding: BasePrivateKeyEncodingOptions<PrivF> & {
    type: "pkcs8";
  };
}

export interface X25519KeyPairOptions<
  PubF extends KeyFormat,
  PrivF extends KeyFormat,
> {
  publicKeyEncoding: {
    type: "spki";
    format: PubF;
  };
  privateKeyEncoding: BasePrivateKeyEncodingOptions<PrivF> & {
    type: "pkcs8";
  };
}

export interface X448KeyPairOptions<
  PubF extends KeyFormat,
  PrivF extends KeyFormat,
> {
  publicKeyEncoding: {
    type: "spki";
    format: PubF;
  };
  privateKeyEncoding: BasePrivateKeyEncodingOptions<PrivF> & {
    type: "pkcs8";
  };
}

export interface RSAKeyPairKeyObjectOptions {
  /**
   * Key size in bits
   */
  modulusLength: number;
  /**
   * Public exponent
   * @default 0x10001
   */
  publicExponent?: number | undefined;
}

export interface RSAPSSKeyPairKeyObjectOptions {
  /**
   * Key size in bits
   */
  modulusLength: number;
  /**
   * Public exponent
   * @default 0x10001
   */
  publicExponent?: number | undefined;
  /**
   * Name of the message digest
   */
  hashAlgorithm?: string;
  /**
   * Name of the message digest used by MGF1
   */
  mgf1HashAlgorithm?: string;
  /**
   * Minimal salt length in bytes
   */
  saltLength?: string;
}

export interface DSAKeyPairKeyObjectOptions {
  /**
   * Key size in bits
   */
  modulusLength: number;
  /**
   * Size of q in bits
   */
  divisorLength: number;
}

// deno-lint-ignore no-empty-interface
export interface ED25519KeyPairKeyObjectOptions {}

// deno-lint-ignore no-empty-interface
export interface ED448KeyPairKeyObjectOptions {}

// deno-lint-ignore no-empty-interface
export interface X25519KeyPairKeyObjectOptions {}

// deno-lint-ignore no-empty-interface
export interface X448KeyPairKeyObjectOptions {}

export interface ECKeyPairKeyObjectOptions {
  /**
   * Name of the curve to use
   */
  namedCurve: string;
}

export function generateKeyPair(
  type: "rsa",
  options: RSAKeyPairOptions<"pem", "pem">,
  callback: (err: Error | null, publicKey: string, privateKey: string) => void,
): void;
export function generateKeyPair(
  type: "rsa",
  options: RSAKeyPairOptions<"pem", "der">,
  callback: (err: Error | null, publicKey: string, privateKey: Buffer) => void,
): void;
export function generateKeyPair(
  type: "rsa",
  options: RSAKeyPairOptions<"der", "pem">,
  callback: (err: Error | null, publicKey: Buffer, privateKey: string) => void,
): void;
export function generateKeyPair(
  type: "rsa",
  options: RSAKeyPairOptions<"der", "der">,
  callback: (err: Error | null, publicKey: Buffer, privateKey: Buffer) => void,
): void;
export function generateKeyPair(
  type: "rsa",
  options: RSAKeyPairKeyObjectOptions,
  callback: (
    err: Error | null,
    publicKey: KeyObject,
    privateKey: KeyObject,
  ) => void,
): void;
export function generateKeyPair(
  type: "rsa-pss",
  options: RSAPSSKeyPairOptions<"pem", "pem">,
  callback: (err: Error | null, publicKey: string, privateKey: string) => void,
): void;
export function generateKeyPair(
  type: "rsa-pss",
  options: RSAPSSKeyPairOptions<"pem", "der">,
  callback: (err: Error | null, publicKey: string, privateKey: Buffer) => void,
): void;
export function generateKeyPair(
  type: "rsa-pss",
  options: RSAPSSKeyPairOptions<"der", "pem">,
  callback: (err: Error | null, publicKey: Buffer, privateKey: string) => void,
): void;
export function generateKeyPair(
  type: "rsa-pss",
  options: RSAPSSKeyPairOptions<"der", "der">,
  callback: (err: Error | null, publicKey: Buffer, privateKey: Buffer) => void,
): void;
export function generateKeyPair(
  type: "rsa-pss",
  options: RSAPSSKeyPairKeyObjectOptions,
  callback: (
    err: Error | null,
    publicKey: KeyObject,
    privateKey: KeyObject,
  ) => void,
): void;
export function generateKeyPair(
  type: "dsa",
  options: DSAKeyPairOptions<"pem", "pem">,
  callback: (err: Error | null, publicKey: string, privateKey: string) => void,
): void;
export function generateKeyPair(
  type: "dsa",
  options: DSAKeyPairOptions<"pem", "der">,
  callback: (err: Error | null, publicKey: string, privateKey: Buffer) => void,
): void;
export function generateKeyPair(
  type: "dsa",
  options: DSAKeyPairOptions<"der", "pem">,
  callback: (err: Error | null, publicKey: Buffer, privateKey: string) => void,
): void;
export function generateKeyPair(
  type: "dsa",
  options: DSAKeyPairOptions<"der", "der">,
  callback: (err: Error | null, publicKey: Buffer, privateKey: Buffer) => void,
): void;
export function generateKeyPair(
  type: "dsa",
  options: DSAKeyPairKeyObjectOptions,
  callback: (
    err: Error | null,
    publicKey: KeyObject,
    privateKey: KeyObject,
  ) => void,
): void;
export function generateKeyPair(
  type: "ec",
  options: ECKeyPairOptions<"pem", "pem">,
  callback: (err: Error | null, publicKey: string, privateKey: string) => void,
): void;
export function generateKeyPair(
  type: "ec",
  options: ECKeyPairOptions<"pem", "der">,
  callback: (err: Error | null, publicKey: string, privateKey: Buffer) => void,
): void;
export function generateKeyPair(
  type: "ec",
  options: ECKeyPairOptions<"der", "pem">,
  callback: (err: Error | null, publicKey: Buffer, privateKey: string) => void,
): void;
export function generateKeyPair(
  type: "ec",
  options: ECKeyPairOptions<"der", "der">,
  callback: (err: Error | null, publicKey: Buffer, privateKey: Buffer) => void,
): void;
export function generateKeyPair(
  type: "ec",
  options: ECKeyPairKeyObjectOptions,
  callback: (
    err: Error | null,
    publicKey: KeyObject,
    privateKey: KeyObject,
  ) => void,
): void;
export function generateKeyPair(
  type: "ed25519",
  options: ED25519KeyPairOptions<"pem", "pem">,
  callback: (err: Error | null, publicKey: string, privateKey: string) => void,
): void;
export function generateKeyPair(
  type: "ed25519",
  options: ED25519KeyPairOptions<"pem", "der">,
  callback: (err: Error | null, publicKey: string, privateKey: Buffer) => void,
): void;
export function generateKeyPair(
  type: "ed25519",
  options: ED25519KeyPairOptions<"der", "pem">,
  callback: (err: Error | null, publicKey: Buffer, privateKey: string) => void,
): void;
export function generateKeyPair(
  type: "ed25519",
  options: ED25519KeyPairOptions<"der", "der">,
  callback: (err: Error | null, publicKey: Buffer, privateKey: Buffer) => void,
): void;
export function generateKeyPair(
  type: "ed25519",
  options: ED25519KeyPairKeyObjectOptions | undefined,
  callback: (
    err: Error | null,
    publicKey: KeyObject,
    privateKey: KeyObject,
  ) => void,
): void;
export function generateKeyPair(
  type: "ed448",
  options: ED448KeyPairOptions<"pem", "pem">,
  callback: (err: Error | null, publicKey: string, privateKey: string) => void,
): void;
export function generateKeyPair(
  type: "ed448",
  options: ED448KeyPairOptions<"pem", "der">,
  callback: (err: Error | null, publicKey: string, privateKey: Buffer) => void,
): void;
export function generateKeyPair(
  type: "ed448",
  options: ED448KeyPairOptions<"der", "pem">,
  callback: (err: Error | null, publicKey: Buffer, privateKey: string) => void,
): void;
export function generateKeyPair(
  type: "ed448",
  options: ED448KeyPairOptions<"der", "der">,
  callback: (err: Error | null, publicKey: Buffer, privateKey: Buffer) => void,
): void;
export function generateKeyPair(
  type: "ed448",
  options: ED448KeyPairKeyObjectOptions | undefined,
  callback: (
    err: Error | null,
    publicKey: KeyObject,
    privateKey: KeyObject,
  ) => void,
): void;
export function generateKeyPair(
  type: "x25519",
  options: X25519KeyPairOptions<"pem", "pem">,
  callback: (err: Error | null, publicKey: string, privateKey: string) => void,
): void;
export function generateKeyPair(
  type: "x25519",
  options: X25519KeyPairOptions<"pem", "der">,
  callback: (err: Error | null, publicKey: string, privateKey: Buffer) => void,
): void;
export function generateKeyPair(
  type: "x25519",
  options: X25519KeyPairOptions<"der", "pem">,
  callback: (err: Error | null, publicKey: Buffer, privateKey: string) => void,
): void;
export function generateKeyPair(
  type: "x25519",
  options: X25519KeyPairOptions<"der", "der">,
  callback: (err: Error | null, publicKey: Buffer, privateKey: Buffer) => void,
): void;
export function generateKeyPair(
  type: "x25519",
  options: X25519KeyPairKeyObjectOptions | undefined,
  callback: (
    err: Error | null,
    publicKey: KeyObject,
    privateKey: KeyObject,
  ) => void,
): void;
export function generateKeyPair(
  type: "x448",
  options: X448KeyPairOptions<"pem", "pem">,
  callback: (err: Error | null, publicKey: string, privateKey: string) => void,
): void;
export function generateKeyPair(
  type: "x448",
  options: X448KeyPairOptions<"pem", "der">,
  callback: (err: Error | null, publicKey: string, privateKey: Buffer) => void,
): void;
export function generateKeyPair(
  type: "x448",
  options: X448KeyPairOptions<"der", "pem">,
  callback: (err: Error | null, publicKey: Buffer, privateKey: string) => void,
): void;
export function generateKeyPair(
  type: "x448",
  options: X448KeyPairOptions<"der", "der">,
  callback: (err: Error | null, publicKey: Buffer, privateKey: Buffer) => void,
): void;
export function generateKeyPair(
  type: "x448",
  options: X448KeyPairKeyObjectOptions | undefined,
  callback: (
    err: Error | null,
    publicKey: KeyObject,
    privateKey: KeyObject,
  ) => void,
): void;
export function generateKeyPair(
  _type: KeyType,
  _options: unknown,
  _callback: (
    err: Error | null,
    // deno-lint-ignore no-explicit-any
    publicKey: any,
    // deno-lint-ignore no-explicit-any
    privateKey: any,
  ) => void,
) {
  notImplemented("crypto.generateKeyPair");
}

export interface KeyPairKeyObjectResult {
  publicKey: KeyObject;
  privateKey: KeyObject;
}

export interface KeyPairSyncResult<
  T1 extends string | Buffer,
  T2 extends string | Buffer,
> {
  publicKey: T1;
  privateKey: T2;
}

export function generateKeyPairSync(
  type: "rsa",
  options: RSAKeyPairOptions<"pem", "pem">,
): KeyPairSyncResult<string, string>;
export function generateKeyPairSync(
  type: "rsa",
  options: RSAKeyPairOptions<"pem", "der">,
): KeyPairSyncResult<string, Buffer>;
export function generateKeyPairSync(
  type: "rsa",
  options: RSAKeyPairOptions<"der", "pem">,
): KeyPairSyncResult<Buffer, string>;
export function generateKeyPairSync(
  type: "rsa",
  options: RSAKeyPairOptions<"der", "der">,
): KeyPairSyncResult<Buffer, Buffer>;
export function generateKeyPairSync(
  type: "rsa",
  options: RSAKeyPairKeyObjectOptions,
): KeyPairKeyObjectResult;
export function generateKeyPairSync(
  type: "rsa-pss",
  options: RSAPSSKeyPairOptions<"pem", "pem">,
): KeyPairSyncResult<string, string>;
export function generateKeyPairSync(
  type: "rsa-pss",
  options: RSAPSSKeyPairOptions<"pem", "der">,
): KeyPairSyncResult<string, Buffer>;
export function generateKeyPairSync(
  type: "rsa-pss",
  options: RSAPSSKeyPairOptions<"der", "pem">,
): KeyPairSyncResult<Buffer, string>;
export function generateKeyPairSync(
  type: "rsa-pss",
  options: RSAPSSKeyPairOptions<"der", "der">,
): KeyPairSyncResult<Buffer, Buffer>;
export function generateKeyPairSync(
  type: "rsa-pss",
  options: RSAPSSKeyPairKeyObjectOptions,
): KeyPairKeyObjectResult;
export function generateKeyPairSync(
  type: "dsa",
  options: DSAKeyPairOptions<"pem", "pem">,
): KeyPairSyncResult<string, string>;
export function generateKeyPairSync(
  type: "dsa",
  options: DSAKeyPairOptions<"pem", "der">,
): KeyPairSyncResult<string, Buffer>;
export function generateKeyPairSync(
  type: "dsa",
  options: DSAKeyPairOptions<"der", "pem">,
): KeyPairSyncResult<Buffer, string>;
export function generateKeyPairSync(
  type: "dsa",
  options: DSAKeyPairOptions<"der", "der">,
): KeyPairSyncResult<Buffer, Buffer>;
export function generateKeyPairSync(
  type: "dsa",
  options: DSAKeyPairKeyObjectOptions,
): KeyPairKeyObjectResult;
export function generateKeyPairSync(
  type: "ec",
  options: ECKeyPairOptions<"pem", "pem">,
): KeyPairSyncResult<string, string>;
export function generateKeyPairSync(
  type: "ec",
  options: ECKeyPairOptions<"pem", "der">,
): KeyPairSyncResult<string, Buffer>;
export function generateKeyPairSync(
  type: "ec",
  options: ECKeyPairOptions<"der", "pem">,
): KeyPairSyncResult<Buffer, string>;
export function generateKeyPairSync(
  type: "ec",
  options: ECKeyPairOptions<"der", "der">,
): KeyPairSyncResult<Buffer, Buffer>;
export function generateKeyPairSync(
  type: "ec",
  options: ECKeyPairKeyObjectOptions,
): KeyPairKeyObjectResult;
export function generateKeyPairSync(
  type: "ed25519",
  options: ED25519KeyPairOptions<"pem", "pem">,
): KeyPairSyncResult<string, string>;
export function generateKeyPairSync(
  type: "ed25519",
  options: ED25519KeyPairOptions<"pem", "der">,
): KeyPairSyncResult<string, Buffer>;
export function generateKeyPairSync(
  type: "ed25519",
  options: ED25519KeyPairOptions<"der", "pem">,
): KeyPairSyncResult<Buffer, string>;
export function generateKeyPairSync(
  type: "ed25519",
  options: ED25519KeyPairOptions<"der", "der">,
): KeyPairSyncResult<Buffer, Buffer>;
export function generateKeyPairSync(
  type: "ed25519",
  options?: ED25519KeyPairKeyObjectOptions,
): KeyPairKeyObjectResult;
export function generateKeyPairSync(
  type: "ed448",
  options: ED448KeyPairOptions<"pem", "pem">,
): KeyPairSyncResult<string, string>;
export function generateKeyPairSync(
  type: "ed448",
  options: ED448KeyPairOptions<"pem", "der">,
): KeyPairSyncResult<string, Buffer>;
export function generateKeyPairSync(
  type: "ed448",
  options: ED448KeyPairOptions<"der", "pem">,
): KeyPairSyncResult<Buffer, string>;
export function generateKeyPairSync(
  type: "ed448",
  options: ED448KeyPairOptions<"der", "der">,
): KeyPairSyncResult<Buffer, Buffer>;
export function generateKeyPairSync(
  type: "ed448",
  options?: ED448KeyPairKeyObjectOptions,
): KeyPairKeyObjectResult;
export function generateKeyPairSync(
  type: "x25519",
  options: X25519KeyPairOptions<"pem", "pem">,
): KeyPairSyncResult<string, string>;
export function generateKeyPairSync(
  type: "x25519",
  options: X25519KeyPairOptions<"pem", "der">,
): KeyPairSyncResult<string, Buffer>;
export function generateKeyPairSync(
  type: "x25519",
  options: X25519KeyPairOptions<"der", "pem">,
): KeyPairSyncResult<Buffer, string>;
export function generateKeyPairSync(
  type: "x25519",
  options: X25519KeyPairOptions<"der", "der">,
): KeyPairSyncResult<Buffer, Buffer>;
export function generateKeyPairSync(
  type: "x25519",
  options?: X25519KeyPairKeyObjectOptions,
): KeyPairKeyObjectResult;
export function generateKeyPairSync(
  type: "x448",
  options: X448KeyPairOptions<"pem", "pem">,
): KeyPairSyncResult<string, string>;
export function generateKeyPairSync(
  type: "x448",
  options: X448KeyPairOptions<"pem", "der">,
): KeyPairSyncResult<string, Buffer>;
export function generateKeyPairSync(
  type: "x448",
  options: X448KeyPairOptions<"der", "pem">,
): KeyPairSyncResult<Buffer, string>;
export function generateKeyPairSync(
  type: "x448",
  options: X448KeyPairOptions<"der", "der">,
): KeyPairSyncResult<Buffer, Buffer>;
export function generateKeyPairSync(
  type: "x448",
  options?: X448KeyPairKeyObjectOptions,
): KeyPairKeyObjectResult;
export function generateKeyPairSync(
  _type: KeyType,
  _options: unknown,
):
  | KeyPairKeyObjectResult
  | KeyPairSyncResult<string | Buffer, string | Buffer> {
  notImplemented("crypto.generateKeyPairSync");
}

export function generateKeySync(
  _type: "hmac" | "aes",
  _options: {
    length: number;
  },
): KeyObject {
  notImplemented("crypto.generateKeySync");
}

export default {
  generateKey,
  generateKeySync,
  generateKeyPair,
  generateKeyPairSync,
};
