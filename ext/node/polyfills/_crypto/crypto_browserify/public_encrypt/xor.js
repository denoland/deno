// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright 2017 Calvin Metcalf. All rights reserved. MIT license.

export function xor(a, b) {
  const len = a.length;
  let i = -1;
  while (++i < len) {
    a[i] ^= b[i];
  }
  return a;
}
