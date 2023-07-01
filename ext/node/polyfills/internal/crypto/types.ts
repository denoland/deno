// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright Joyent, Inc. and Node.js contributors. All rights reserved. MIT license.

import { Buffer } from "../../buffer.ts";

export type HASH_DATA = string | ArrayBufferView | Buffer;

export type BinaryToTextEncoding = "base64" | "base64url" | "hex" | "binary";

export type CharacterEncoding = "utf8" | "utf-8" | "utf16le" | "latin1";

export type LegacyCharacterEncoding = "ascii" | "binary" | "ucs2" | "ucs-2";

export type Encoding =
  | BinaryToTextEncoding
  | CharacterEncoding
  | LegacyCharacterEncoding;

export type ECDHKeyFormat = "compressed" | "uncompressed" | "hybrid";

export type BinaryLike = string | ArrayBufferView;

export type KeyFormat = "pem" | "der";

export type KeyType =
  | "rsa"
  | "rsa-pss"
  | "dsa"
  | "ec"
  | "ed25519"
  | "ed448"
  | "x25519"
  | "x448";

export interface PrivateKeyInput {
  key: string | Buffer;
  format?: KeyFormat | undefined;
  type?: "pkcs1" | "pkcs8" | "sec1" | undefined;
  passphrase?: string | Buffer | undefined;
}

export interface PublicKeyInput {
  key: string | Buffer;
  format?: KeyFormat | undefined;
  type?: "pkcs1" | "spki" | undefined;
}
