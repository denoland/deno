// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

import { fnv32, fnv32a } from "./fnv32.ts";
import { fnv64, fnv64a } from "./fnv64.ts";

export function fnv(name: string, buf?: Uint8Array): ArrayBuffer {
  if (!buf) {
    throw new TypeError("no data provided for hashing");
  }

  switch (name) {
    case "FNV32":
      return fnv32(buf);
    case "FNV64":
      return fnv64(buf);
    case "FNV32A":
      return fnv32a(buf);
    case "FNV64A":
      return fnv64a(buf);
    default:
      throw new TypeError(`unsupported fnv digest: ${name}`);
  }
}
