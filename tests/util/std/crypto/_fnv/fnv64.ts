// Ported from Go:
// https://github.com/golang/go/tree/go1.13.10/src/hash/fnv/fnv.go
// Copyright 2011 The Go Authors. All rights reserved. BSD license.
// https://github.com/golang/go/blob/master/LICENSE
// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This module is browser compatible.

import { mul64, swap32 } from "./util.ts";

const prime64Lo = 435;
const prime64Hi = 256;

export const fnv64 = (data: Uint8Array): ArrayBuffer => {
  let hashLo = 2216829733;
  let hashHi = 3421674724;

  data.forEach((c) => {
    [hashHi, hashLo] = mul64([hashHi, hashLo], [prime64Hi, prime64Lo]);
    hashLo ^= c;
  });

  return new Uint32Array([swap32(hashHi >>> 0), swap32(hashLo >>> 0)]).buffer;
};

export const fnv64a = (data: Uint8Array): ArrayBuffer => {
  let hashLo = 2216829733;
  let hashHi = 3421674724;

  data.forEach((c) => {
    hashLo ^= c;
    [hashHi, hashLo] = mul64([hashHi, hashLo], [prime64Hi, prime64Lo]);
  });

  return new Uint32Array([swap32(hashHi >>> 0), swap32(hashLo >>> 0)]).buffer;
};
