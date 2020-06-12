// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import {
  bytesToUuid,
  createBuffer,
  stringToBytes,
  uuidToBytes,
} from "./_common.ts";
import { Sha1 } from "../hash/sha1.ts";
import { isString } from "../node/util.ts";
import { assert } from "../_util/assert.ts";

const UUID_RE = /^[0-9a-f]{8}-[0-9a-f]{4}-[5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i;

export function validate(id: string): boolean {
  return UUID_RE.test(id);
}

interface V5Options {
  value: string | number[];
  namespace: string | number[];
}

export function generate(
  options: V5Options,
  buf?: number[],
  offset?: number
): string | number[] {
  const i = (buf && offset) || 0;

  let { value, namespace } = options;
  if (isString(value)) value = stringToBytes(value as string);
  if (isString(namespace)) namespace = uuidToBytes(namespace as string);
  assert(
    namespace.length === 16,
    "namespace must be uuid string or an Array of 16 byte values"
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
