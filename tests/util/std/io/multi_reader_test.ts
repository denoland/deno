// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { assertEquals } from "../assert/mod.ts";
import { MultiReader } from "./multi_reader.ts";
import { StringWriter } from "./string_writer.ts";
import { copyN } from "./copy_n.ts";
import { copy } from "../streams/copy.ts";
import { StringReader } from "./string_reader.ts";

Deno.test("ioMultiReader", async function () {
  const r = new MultiReader([new StringReader("abc"), new StringReader("def")]);
  const w = new StringWriter();
  const n = await copyN(r, w, 4);
  assertEquals(n, 4);
  assertEquals(w.toString(), "abcd");
  await copy(r, w);
  assertEquals(w.toString(), "abcdef");
});
