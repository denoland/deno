const { copy } = Deno;
import { test } from "../testing/mod.ts";
import { assertEquals } from "../testing/asserts.ts";
import { MultiReader, StringReader } from "./readers.ts";
import { StringWriter } from "./writers.ts";
import { copyN } from "./ioutil.ts";
import { decode } from "../strings/strings.ts";

test(async function ioStringReader() {
  const r = new StringReader("abcdef");
  const { nread, eof } = await r.read(new Uint8Array(6));
  assertEquals(nread, 6);
  assertEquals(eof, true);
});

test(async function ioStringReader() {
  const r = new StringReader("abcdef");
  const buf = new Uint8Array(3);
  let res1 = await r.read(buf);
  assertEquals(res1.nread, 3);
  assertEquals(res1.eof, false);
  assertEquals(decode(buf), "abc");
  let res2 = await r.read(buf);
  assertEquals(res2.nread, 3);
  assertEquals(res2.eof, true);
  assertEquals(decode(buf), "def");
});

test(async function ioMultiReader() {
  const r = new MultiReader(new StringReader("abc"), new StringReader("def"));
  const w = new StringWriter();
  const n = await copyN(w, r, 4);
  assertEquals(n, 4);
  assertEquals(w.toString(), "abcd");
  await copy(w, r);
  assertEquals(w.toString(), "abcdef");
});
