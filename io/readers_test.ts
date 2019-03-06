const { copy } = Deno;
import { test } from "../testing/mod.ts";
import { assertEq } from "../testing/asserts.ts";
import { MultiReader, StringReader } from "./readers.ts";
import { StringWriter } from "./writers.ts";
import { copyN } from "./ioutil.ts";
import { decode } from "../strings/strings.ts";

test(async function ioStringReader() {
  const r = new StringReader("abcdef");
  const { nread, eof } = await r.read(new Uint8Array(6));
  assertEq(nread, 6);
  assertEq(eof, true);
});

test(async function ioStringReader() {
  const r = new StringReader("abcdef");
  const buf = new Uint8Array(3);
  let res1 = await r.read(buf);
  assertEq(res1.nread, 3);
  assertEq(res1.eof, false);
  assertEq(decode(buf), "abc");
  let res2 = await r.read(buf);
  assertEq(res2.nread, 3);
  assertEq(res2.eof, true);
  assertEq(decode(buf), "def");
});

test(async function ioMultiReader() {
  const r = new MultiReader(new StringReader("abc"), new StringReader("def"));
  const w = new StringWriter();
  const n = await copyN(w, r, 4);
  assertEq(n, 4);
  assertEq(w.toString(), "abcd");
  await copy(w, r);
  assertEq(w.toString(), "abcdef");
});
