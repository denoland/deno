// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright Joyent, Inc. and Node.js contributors. All rights reserved. MIT license.

import { KeyObject } from "internal:deno_node/internal/crypto/keys.ts";
import { Buffer } from "internal:deno_node/buffer.ts";
import { ERR_INVALID_ARG_TYPE } from "internal:deno_node/internal/errors.ts";
import { isArrayBufferView } from "internal:deno_node/internal/util/types.ts";
import { notImplemented } from "internal:deno_node/_utils.ts";
import { BinaryLike } from "internal:deno_node/internal/crypto/types.ts";

// deno-lint-ignore no-explicit-any
export type PeerCertificate = any;

export interface X509CheckOptions {
  /**
   * @default 'always'
   */
  subject: "always" | "never";
  /**
   * @default true
   */
  wildcards: boolean;
  /**
   * @default true
   */
  partialWildcards: boolean;
  /**
   * @default false
   */
  multiLabelWildcards: boolean;
  /**
   * @default false
   */
  singleLabelSubdomains: boolean;
}

export class X509Certificate {
  constructor(buffer: BinaryLike) {
    if (typeof buffer === "string") {
      buffer = Buffer.from(buffer);
    }

    if (!isArrayBufferView(buffer)) {
      throw new ERR_INVALID_ARG_TYPE(
        "buffer",
        ["string", "Buffer", "TypedArray", "DataView"],
        buffer,
      );
    }

    notImplemented("crypto.X509Certificate");
  }

  get ca(): boolean {
    notImplemented("crypto.X509Certificate.prototype.ca");

    return false;
  }

  checkEmail(
    _email: string,
    _options?: Pick<X509CheckOptions, "subject">,
  ): string | undefined {
    notImplemented("crypto.X509Certificate.prototype.checkEmail");
  }

  checkHost(_name: string, _options?: X509CheckOptions): string | undefined {
    notImplemented("crypto.X509Certificate.prototype.checkHost");
  }

  checkIP(_ip: string): string | undefined {
    notImplemented("crypto.X509Certificate.prototype.checkIP");
  }

  checkIssued(_otherCert: X509Certificate): boolean {
    notImplemented("crypto.X509Certificate.prototype.checkIssued");
  }

  checkPrivateKey(_privateKey: KeyObject): boolean {
    notImplemented("crypto.X509Certificate.prototype.checkPrivateKey");
  }

  get fingerprint(): string {
    notImplemented("crypto.X509Certificate.prototype.fingerprint");

    return "";
  }

  get fingerprint256(): string {
    notImplemented("crypto.X509Certificate.prototype.fingerprint256");

    return "";
  }

  get fingerprint512(): string {
    notImplemented("crypto.X509Certificate.prototype.fingerprint512");

    return "";
  }

  get infoAccess(): string | undefined {
    notImplemented("crypto.X509Certificate.prototype.infoAccess");

    return "";
  }

  get issuer(): string {
    notImplemented("crypto.X509Certificate.prototype.issuer");

    return "";
  }

  get issuerCertificate(): X509Certificate | undefined {
    notImplemented("crypto.X509Certificate.prototype.issuerCertificate");

    return {} as X509Certificate;
  }

  get keyUsage(): string[] {
    notImplemented("crypto.X509Certificate.prototype.keyUsage");

    return [];
  }

  get publicKey(): KeyObject {
    notImplemented("crypto.X509Certificate.prototype.publicKey");

    return {} as KeyObject;
  }

  get raw(): Buffer {
    notImplemented("crypto.X509Certificate.prototype.raw");

    return {} as Buffer;
  }

  get serialNumber(): string {
    notImplemented("crypto.X509Certificate.prototype.serialNumber");

    return "";
  }

  get subject(): string {
    notImplemented("crypto.X509Certificate.prototype.subject");

    return "";
  }

  get subjectAltName(): string | undefined {
    notImplemented("crypto.X509Certificate.prototype.subjectAltName");

    return "";
  }

  toJSON(): string {
    return this.toString();
  }

  toLegacyObject(): PeerCertificate {
    notImplemented("crypto.X509Certificate.prototype.toLegacyObject");
  }

  toString(): string {
    notImplemented("crypto.X509Certificate.prototype.toString");
  }

  get validFrom(): string {
    notImplemented("crypto.X509Certificate.prototype.validFrom");

    return "";
  }

  get validTo(): string {
    notImplemented("crypto.X509Certificate.prototype.validTo");

    return "";
  }

  verify(_publicKey: KeyObject): boolean {
    notImplemented("crypto.X509Certificate.prototype.verify");
  }
}

export default {
  X509Certificate,
};
