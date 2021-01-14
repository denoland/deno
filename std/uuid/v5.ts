// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import {
  bytesToUuid,
  createBuffer,
  stringToBytes,
  uuidToBytes,
} from "./_common.ts";
import { Sha1 } from "../hash/sha1.ts";
import { assert } from "../_util/assert.ts";

const UUID_RE =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i;

/**
 * Validates the UUID v5
 * @param id UUID value
 */
export function validate(id: string): boolean {
  return UUID_RE.test(id);
}

interface V5Options {
  value: string | number[];
  namespace: string | number[];
}

/**
 * Generates a RFC4122 v5 UUID (SHA-1 namespace-based)
 * @param options Can use a namespace and value to create SHA-1 hash
 * @param buf Can allow the UUID to be written in byte-form starting at the offset
 * @param offset Index to start writing on the UUID bytes in buffer
 */
export function generate(
  options: V5Options,
  buf?: number[],
  offset?: number,
): string | number[] {
  const i = (buf && offset) || 0;

  let { value, namespace } = options;
  if (typeof value == "string") {
    value = stringToBytes(value as string);
  }

  if (typeof namespace == "string") {
    namespace = uuidToBytes(namespace as string);
  }

  assert(
    namespace.length === 16,
    "namespace must be uuid string or an Array of 16 byte values",
  );

  const content = (namespace as number[]).concat(value as number[]);
  const bytes = new Sha1().update(createBuffer(content)).digest();

  bytes[6] = (bytes[6] & 0x0f) | 0x50;
  bytes[8] = (bytes[8] & 0x3f) | 0x80;

  if (buf) {
    for (let idx = 0; idx < 16; ++idx) {
      buf[i + idx] = bytes[idx];
    }
  }

  return buf || bytesToUuid(bytes);
}
