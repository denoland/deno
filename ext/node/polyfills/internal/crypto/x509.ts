// Copyright 2018-2026 the Deno authors. MIT license.
// Copyright Joyent, Inc. and Node.js contributors. All rights reserved. MIT license.

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file prefer-primordials

import {
  op_node_x509_ca,
  op_node_x509_check_email,
  op_node_x509_check_host,
  op_node_x509_check_ip,
  op_node_x509_check_issued,
  op_node_x509_check_private_key,
  op_node_x509_fingerprint,
  op_node_x509_fingerprint256,
  op_node_x509_fingerprint512,
  op_node_x509_get_info_access,
  op_node_x509_get_issuer,
  op_node_x509_get_raw,
  op_node_x509_get_serial_number,
  op_node_x509_get_signature_algorithm_name,
  op_node_x509_get_signature_algorithm_oid,
  op_node_x509_get_subject,
  op_node_x509_get_subject_alt_name,
  op_node_x509_get_valid_from,
  op_node_x509_get_valid_to,
  op_node_x509_key_usage,
  op_node_x509_parse,
  op_node_x509_public_key,
  op_node_x509_to_legacy_object,
  op_node_x509_to_string,
  op_node_x509_verify,
} from "ext:core/ops";

import {
  KeyObject,
  PublicKeyObject,
} from "ext:deno_node/internal/crypto/keys.ts";
import { kHandle } from "ext:deno_node/internal/crypto/constants.ts";
import { Buffer } from "node:buffer";
import {
  ERR_INVALID_ARG_TYPE,
  ERR_INVALID_ARG_VALUE,
} from "ext:deno_node/internal/errors.ts";
import { isArrayBufferView } from "ext:deno_node/internal/util/types.ts";
import {
  validateBoolean,
  validateObject,
  validateString,
} from "ext:deno_node/internal/validators.mjs";
import type { BinaryLike } from "ext:deno_node/internal/crypto/types.ts";
import { inspect } from "node:util";
import { customInspectSymbol as kInspect } from "ext:deno_node/internal/util.mjs";
import type { InspectOptions } from "node:util";

const core = globalThis.Deno.core;

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

// deno-lint-ignore no-explicit-any
const kEmptyObject = Object.freeze({ __proto__: null } as any);

function getFlags(options = kEmptyObject): number {
  validateObject(options, "options");
  const {
    subject = "default",
    wildcards = true,
    partialWildcards = true,
    multiLabelWildcards = false,
    singleLabelSubdomains = false,
  } = { ...options };
  let flags = 0;
  validateString(subject, "options.subject");
  validateBoolean(wildcards, "options.wildcards");
  validateBoolean(partialWildcards, "options.partialWildcards");
  validateBoolean(multiLabelWildcards, "options.multiLabelWildcards");
  validateBoolean(singleLabelSubdomains, "options.singleLabelSubdomains");
  switch (subject) {
    case "default":
      break;
    case "always":
      flags |= 0x1;
      break;
    case "never":
      flags |= 0x2;
      break;
    default:
      throw new ERR_INVALID_ARG_VALUE("options.subject", subject);
  }
  // Flags are parsed for validation but not currently used
  // as the underlying implementation doesn't use OpenSSL's
  // X509_check_* functions.
  if (!wildcards) flags |= 0x4;
  if (!partialWildcards) flags |= 0x8;
  if (multiLabelWildcards) flags |= 0x10;
  if (singleLabelSubdomains) flags |= 0x20;
  return flags;
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

    this.#handle = op_node_x509_parse(buffer);
    // deno-lint-ignore no-this-alias
    const self = this;
    this[core.hostObjectBrand] = () => ({
      type: "X509Certificate",
      data: op_node_x509_get_raw(self.#handle),
    });
  }

  [kInspect](depth: number, options: InspectOptions) {
    if (depth < 0) {
      return this;
    }

    const opts = {
      ...options,
      depth: options.depth == null ? null : options.depth - 1,
    };

    return `X509Certificate ${
      inspect({
        subject: this.subject,
        subjectAltName: this.subjectAltName,
        issuer: this.issuer,
        infoAccess: this.infoAccess,
        validFrom: this.validFrom,
        validTo: this.validTo,
        validFromDate: this.validFromDate,
        validToDate: this.validToDate,
        fingerprint: this.fingerprint,
        fingerprint256: this.fingerprint256,
        fingerprint512: this.fingerprint512,
        keyUsage: this.keyUsage,
        serialNumber: this.serialNumber,
      }, opts)
    }`;
  }

  get ca(): boolean {
    return op_node_x509_ca(this.#handle);
  }

  checkEmail(
    email: string,
    options?: Pick<X509CheckOptions, "subject">,
  ): string | undefined {
    validateString(email, "email");
    if (email.includes("\0")) {
      throw new ERR_INVALID_ARG_VALUE("email", email);
    }
    getFlags(options);
    if (op_node_x509_check_email(this.#handle, email)) {
      return email;
    }
  }

  checkHost(name: string, options?: X509CheckOptions): string | undefined {
    validateString(name, "name");
    if (name.includes("\0")) {
      throw new ERR_INVALID_ARG_VALUE("name", name);
    }
    getFlags(options);
    if (op_node_x509_check_host(this.#handle, name)) {
      return name;
    }
  }

  checkIP(ip: string, options?: unknown): string | undefined {
    validateString(ip, "ip");
    getFlags(options);
    return op_node_x509_check_ip(this.#handle, ip) ?? undefined;
  }

  checkIssued(otherCert: X509Certificate): boolean {
    if (!(otherCert instanceof X509Certificate)) {
      throw new ERR_INVALID_ARG_TYPE(
        "otherCert",
        "X509Certificate",
        otherCert,
      );
    }
    return op_node_x509_check_issued(this.#handle, otherCert.#handle);
  }

  checkPrivateKey(privateKey: KeyObject): boolean {
    if (!(privateKey instanceof KeyObject)) {
      throw new ERR_INVALID_ARG_TYPE(
        "privateKey",
        "KeyObject",
        privateKey,
      );
    }
    if (privateKey.type !== "private") {
      throw new ERR_INVALID_ARG_VALUE("privateKey", privateKey);
    }
    return op_node_x509_check_private_key(
      this.#handle,
      // deno-lint-ignore no-explicit-any
      (privateKey as any)[kHandle],
    );
  }

  get fingerprint(): string {
    return op_node_x509_fingerprint(this.#handle);
  }

  get fingerprint256(): string {
    return op_node_x509_fingerprint256(this.#handle);
  }

  get fingerprint512(): string {
    return op_node_x509_fingerprint512(this.#handle);
  }

  get infoAccess(): string | undefined {
    return op_node_x509_get_info_access(this.#handle) ?? undefined;
  }

  get issuer(): string {
    return op_node_x509_get_issuer(this.#handle);
  }

  get issuerCertificate(): X509Certificate | undefined {
    return undefined;
  }

  get keyUsage(): string[] | undefined {
    const flags = op_node_x509_key_usage(this.#handle);
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

  get publicKey(): PublicKeyObject {
    const handle = op_node_x509_public_key(this.#handle);
    return new PublicKeyObject(handle);
  }

  get raw(): Buffer {
    return Buffer.from(op_node_x509_get_raw(this.#handle));
  }

  get serialNumber(): string {
    return op_node_x509_get_serial_number(this.#handle);
  }

  get signatureAlgorithm(): string | undefined {
    return op_node_x509_get_signature_algorithm_name(this.#handle) ?? undefined;
  }

  get signatureAlgorithmOid(): string {
    return op_node_x509_get_signature_algorithm_oid(this.#handle);
  }

  get subject(): string {
    return op_node_x509_get_subject(this.#handle) || undefined;
  }

  get subjectAltName(): string | undefined {
    return op_node_x509_get_subject_alt_name(this.#handle) ?? undefined;
  }

  toJSON(): string {
    return this.toString();
  }

  toLegacyObject(): PeerCertificate {
    const obj = op_node_x509_to_legacy_object(this.#handle);
    if (obj.raw) {
      obj.raw = Buffer.from(obj.raw);
    }
    if (obj.subject) {
      obj.subject = Object.assign({ __proto__: null }, obj.subject);
    }
    if (obj.issuer) {
      obj.issuer = Object.assign({ __proto__: null }, obj.issuer);
    }
    if (obj.infoAccess) {
      obj.infoAccess = Object.assign({ __proto__: null }, obj.infoAccess);
    }
    return obj;
  }

  toString(): string {
    return op_node_x509_to_string(this.#handle);
  }

  get validFrom(): string {
    return op_node_x509_get_valid_from(this.#handle);
  }

  get validFromDate(): Date {
    return new Date(this.validFrom);
  }

  get validTo(): string {
    return op_node_x509_get_valid_to(this.#handle);
  }

  get validToDate(): Date {
    return new Date(this.validTo);
  }

  verify(publicKey: KeyObject): boolean {
    if (!(publicKey instanceof KeyObject)) {
      throw new ERR_INVALID_ARG_TYPE(
        "publicKey",
        "KeyObject",
        publicKey,
      );
    }
    if (publicKey.type !== "public") {
      throw new ERR_INVALID_ARG_VALUE("publicKey", publicKey);
    }
    return op_node_x509_verify(
      this.#handle,
      // deno-lint-ignore no-explicit-any
      (publicKey as any)[kHandle],
    );
  }
}

function isX509Certificate(value: unknown): value is X509Certificate {
  return value instanceof X509Certificate;
}

core.registerCloneableResource(
  "X509Certificate",
  (data: { data: ArrayBuffer }) => new X509Certificate(Buffer.from(data.data)),
);

export default {
  X509Certificate,
  isX509Certificate,
};
