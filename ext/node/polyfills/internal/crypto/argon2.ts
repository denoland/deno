// Copyright 2018-2026 the Deno authors. MIT license.

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file prefer-primordials

import { op_node_argon2_async, op_node_argon2_sync } from "ext:core/ops";
import { Buffer } from "node:buffer";
import {
  isAnyArrayBuffer,
  isArrayBufferView,
} from "ext:deno_node/internal/util/types.ts";
import { NodeError } from "ext:deno_node/internal/errors.ts";

const TWO_POW_24 = 16777216;
const TWO_POW_32 = 4294967296;

const REQUIRED_KEYS = [
  "message",
  "nonce",
  "parallelism",
  "tagLength",
  "memory",
  "passes",
] as const;

type Argon2Algorithm = "argon2d" | "argon2i" | "argon2id";

interface NormalizedParams {
  message: Uint8Array;
  nonce: Uint8Array;
  secret: Uint8Array | undefined;
  associatedData: Uint8Array | undefined;
  parallelism: number;
  tagLength: number;
  memory: number;
  passes: number;
}

function toBytes(value: unknown, name: string): Uint8Array {
  if (typeof value === "string") {
    return new Uint8Array(Buffer.from(value, "utf8"));
  }
  if (isArrayBufferView(value)) {
    const view = value as ArrayBufferView;
    return new Uint8Array(view.buffer, view.byteOffset, view.byteLength);
  }
  if (isAnyArrayBuffer(value)) {
    return new Uint8Array(value as ArrayBuffer);
  }
  throw new NodeError(
    "ERR_INVALID_ARG_TYPE",
    `The "${name}" argument must be of type string or an instance of ` +
      `Buffer, TypedArray, or DataView. Received ${
        value === null ? "null" : typeof value
      }`,
  );
}

function validateRange(
  value: unknown,
  name: string,
  min: number,
  maxExclusive: number,
): number {
  if (
    typeof value !== "number" || !Number.isInteger(value) || value < min ||
    value >= maxExclusive
  ) {
    throw new NodeError(
      "ERR_OUT_OF_RANGE",
      `The value of "${name}" is out of range. It must be >= ${min} and ` +
        `< ${maxExclusive}. Received ${value}`,
    );
  }
  return value;
}

function validateAlgorithm(algorithm: unknown): Argon2Algorithm {
  if (typeof algorithm !== "string") {
    throw new NodeError(
      "ERR_INVALID_ARG_TYPE",
      `The "algorithm" argument must be of type string. Received ` +
        `${algorithm === null ? "null" : typeof algorithm}`,
    );
  }
  if (
    algorithm !== "argon2d" && algorithm !== "argon2i" &&
    algorithm !== "argon2id"
  ) {
    throw new NodeError(
      "ERR_INVALID_ARG_VALUE",
      `Invalid Argon2 algorithm: ${algorithm}`,
    );
  }
  return algorithm;
}

function validateParams(parameters: unknown): NormalizedParams {
  if (
    parameters === null || parameters === undefined ||
    typeof parameters !== "object"
  ) {
    throw new NodeError(
      "ERR_INVALID_ARG_TYPE",
      `The "parameters" argument must be of type object. Received ` +
        `${parameters === null ? "null" : typeof parameters}`,
    );
  }
  const params = parameters as Record<string, unknown>;
  for (const key of REQUIRED_KEYS) {
    if (!(key in params)) {
      throw new NodeError(
        "ERR_INVALID_ARG_TYPE",
        `The "parameters.${key}" property must be defined.`,
      );
    }
  }

  const message = toBytes(params.message, "parameters.message");
  const nonce = toBytes(params.nonce, "parameters.nonce");
  if (nonce.byteLength < 8) {
    throw new NodeError(
      "ERR_OUT_OF_RANGE",
      `The value of "parameters.nonce.byteLength" is out of range. It must ` +
        `be >= 8. Received ${nonce.byteLength}`,
    );
  }

  const secret = params.secret === undefined
    ? undefined
    : toBytes(params.secret, "parameters.secret");
  const associatedData = params.associatedData === undefined
    ? undefined
    : toBytes(params.associatedData, "parameters.associatedData");

  const tagLength = validateRange(
    params.tagLength,
    "parameters.tagLength",
    4,
    TWO_POW_32,
  );
  const passes = validateRange(
    params.passes,
    "parameters.passes",
    1,
    TWO_POW_32,
  );
  const parallelism = validateRange(
    params.parallelism,
    "parameters.parallelism",
    1,
    TWO_POW_24,
  );
  const memory = validateRange(
    params.memory,
    "parameters.memory",
    0,
    TWO_POW_32,
  );
  if (memory < 8 * parallelism) {
    throw new NodeError(
      "ERR_OUT_OF_RANGE",
      `The value of "parameters.memory" is out of range. It must be >= ` +
        `${8 * parallelism} (8 * parallelism). Received ${memory}`,
    );
  }

  return {
    message,
    nonce,
    secret,
    associatedData,
    parallelism,
    tagLength,
    memory,
    passes,
  };
}

export function argon2Sync(
  algorithm: unknown,
  parameters: unknown,
): Buffer {
  const alg = validateAlgorithm(algorithm);
  const v = validateParams(parameters);
  const result = op_node_argon2_sync(
    alg,
    v.message,
    v.nonce,
    v.secret,
    v.associatedData,
    v.parallelism,
    v.tagLength,
    v.memory,
    v.passes,
  );
  return Buffer.from(result.buffer, result.byteOffset, result.byteLength);
}

export function argon2(
  algorithm: unknown,
  parameters: unknown,
  callback: unknown,
): void {
  if (typeof callback !== "function") {
    throw new NodeError(
      "ERR_INVALID_ARG_TYPE",
      `The "callback" argument must be of type function. Received ` +
        `${callback === null ? "null" : typeof callback}`,
    );
  }
  const alg = validateAlgorithm(algorithm);
  const v = validateParams(parameters);
  op_node_argon2_async(
    alg,
    v.message,
    v.nonce,
    v.secret,
    v.associatedData,
    v.parallelism,
    v.tagLength,
    v.memory,
    v.passes,
  ).then(
    (result: Uint8Array) => {
      const buf = Buffer.from(
        result.buffer,
        result.byteOffset,
        result.byteLength,
      );
      (callback as (err: Error | null, result?: Buffer) => void)(null, buf);
    },
    (err: Error) => {
      (callback as (err: Error | null, result?: Buffer) => void)(err);
    },
  );
}

export default { argon2, argon2Sync };
