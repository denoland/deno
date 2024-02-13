// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// This code has been ported almost directly from Go's src/bytes/buffer_test.go
// Copyright 2009 The Go Authors. All rights reserved. BSD license.
// https://github.com/golang/go/blob/master/LICENSE
import { assertEquals } from "../assert/mod.ts";
import { readStringDelim } from "./read_string_delim.ts";
import { StringReader } from "./string_reader.ts";

Deno.test("[io] readStringDelim basic", async () => {
  const delim = "!#$%&()=~";
  const exp = [
    "",
    "a",
    "bc",
    "def",
    "",
    "!",
    "!#",
    "!#$%&()=",
    "#$%&()=~",
    "",
    "",
  ];
  const str = exp.join(delim);
  const arr: string[] = [];
  for await (const v of readStringDelim(new StringReader(str), delim)) {
    arr.push(v);
  }
  assertEquals(arr, exp);
});

Deno.test("[io] readStringDelim bigger delim than buf size", async () => {
  // 0123456789...
  const delim = Array.from({ length: 1025 }).map((_, i) => i % 10).join("");
  const exp = ["", "a", "bc", "def", "01", "012345678", "123456789", "", ""];
  const str = exp.join(delim);
  const arr: string[] = [];
  for await (const v of readStringDelim(new StringReader(str), delim)) {
    arr.push(v);
  }
  assertEquals(arr, exp);
});

Deno.test("[io] readStringDelim delim=1213", async () => {
  const delim = "1213";
  const exp = ["", "a", "bc", "def", "01", "012345678", "123456789", "", ""];
  const str = exp.join(delim);
  const arr: string[] = [];
  for await (const v of readStringDelim(new StringReader(str), "1213")) {
    arr.push(v);
  }
  assertEquals(arr, exp);
});
