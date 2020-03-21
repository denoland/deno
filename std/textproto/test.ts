// Based on https://github.com/golang/go/blob/891682/src/net/textproto/
// Copyright 2009 The Go Authors. All rights reserved.
// Use of this source code is governed by a BSD-style
// license that can be found in the LICENSE file.

import { append } from "./mod.ts";
import { assertEquals } from "../testing/asserts.ts";
const { test } = Deno;

test(function textprotoAppend(): void {
  const enc = new TextEncoder();
  const dec = new TextDecoder();
  const u1 = enc.encode("Hello ");
  const u2 = enc.encode("World");
  const joined = append(u1, u2);
  assertEquals(dec.decode(joined), "Hello World");
});
