// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
// Copyright Joyent, Inc. and Node.js contributors. All rights reserved. MIT license.

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file prefer-primordials

import { KeyObject } from "ext:deno_node/internal/crypto/keys.ts";
import { Buffer } from "node:buffer";
import { ERR_INVALID_ARG_TYPE } from "ext:deno_node/internal/errors.ts";
import { isArrayBufferView } from "ext:deno_node/internal/util/types.ts";
import { validateString } from "ext:deno_node/internal/validators.mjs";
import { notImplemented } from "ext:deno_node/_utils.ts";
import { BinaryLike } from "ext:deno_node/internal/crypto/types.ts";

const { ops } = globalThis.__bootstrap.core;

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
  #handle: number;

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

    this.#handle = ops.op_node_x509_parse(buffer);
  }

  get ca(): boolean {
    return ops.op_node_x509_ca(this.#handle);
  }

  checkEmail(
    email: string,
    _options?: Pick<X509CheckOptions, "subject">,
  ): string | undefined {
    validateString(email, "email");
    if (ops.op_node_x509_check_email(this.#handle, email)) {
      return email;
    }
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
    return ops.op_node_x509_fingerprint(this.#handle);
  }

  get fingerprint256(): string {
    return ops.op_node_x509_fingerprint256(this.#handle);
  }

  get fingerprint512(): string {
    return ops.op_node_x509_fingerprint512(this.#handle);
  }

  get infoAccess(): string | undefined {
    notImplemented("crypto.X509Certificate.prototype.infoAccess");

    return "";
  }

  get issuer(): string {
    return ops.op_node_x509_get_issuer(this.#handle);
  }

  get issuerCertificate(): X509Certificate | undefined {
    return undefined;
  }

  get keyUsage(): string[] | undefined {
    const flags = ops.op_node_x509_key_usage(this.#handle);
    if (flags === 0) return undefined;
    const result: string[] = [];
    if (flags & 0x01) result.push("DigitalSignature");
    if (flags >> 1 & 0x01) result.push("NonRepudiation");
    if (flags >> 2 & 0x01) result.push("KeyEncipherment");
    if (flags >> 3 & 0x01) result.push("DataEncipherment");
    if (flags >> 4 & 0x01) result.push("KeyAgreement");
    if (flags >> 5 & 0x01) result.push("KeyCertSign");
    if (flags >> 6 & 0x01) result.push("CRLSign");
    if (flags >> 7 & 0x01) result.push("EncipherOnly");
    if (flags >> 8 & 0x01) result.push("DecipherOnly");
    return result;
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
    return ops.op_node_x509_get_serial_number(this.#handle);
  }

  get subject(): string {
    return ops.op_node_x509_get_subject(this.#handle);
  }

  get subjectAltName(): string | undefined {
    return undefined;
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
    return ops.op_node_x509_get_valid_from(this.#handle);
  }

  get validTo(): string {
    return ops.op_node_x509_get_valid_to(this.#handle);
  }

  verify(_publicKey: KeyObject): boolean {
    notImplemented("crypto.X509Certificate.prototype.verify");
  }
}

export default {
  X509Certificate,
};
