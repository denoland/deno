// Ported from Go:
// https://github.com/golang/go/tree/go1.13.10/src/hash/fnv/fnv.go
// Copyright 2011 The Go Authors. All rights reserved. BSD license.
// https://github.com/golang/go/blob/master/LICENSE
// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

import { mul32, swap32 } from "./util.ts";

const prime32 = 16777619;

export const fnv32 = (data: Uint8Array): ArrayBuffer => {
  let hash = 2166136261;

  data.forEach((c) => {
    hash = mul32(hash, prime32);
    hash ^= c;
  });

  return Uint32Array.from([swap32(hash)]).buffer;
};

export const fnv32a = (data: Uint8Array): ArrayBuffer => {
  let hash = 2166136261;

  data.forEach((c) => {
    hash ^= c;
    hash = mul32(hash, prime32);
  });

  return Uint32Array.from([swap32(hash)]).buffer;
};
