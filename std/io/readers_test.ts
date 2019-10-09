const { copy } = Deno;
import { test } from "../testing/mod.ts";
import { assertEquals } from "../testing/asserts.ts";
import { MultiReader, StringReader } from "./readers.ts";
import { StringWriter } from "./writers.ts";
import { copyN } from "./ioutil.ts";
import { decode } from "../strings/mod.ts";

test(async function ioStringReader(): Promise<void> {
  const r = new StringReader("abcdef");
  const res0 = await r.read(new Uint8Array(6));
  assertEquals(res0, 6);
  const res1 = await r.read(new Uint8Array(6));
  assertEquals(res1, Deno.EOF);
});

test(async function ioStringReader(): Promise<void> {
  const r = new StringReader("abcdef");
  const buf = new Uint8Array(3);
  const res1 = await r.read(buf);
  assertEquals(res1, 3);
  assertEquals(decode(buf), "abc");
  const res2 = await r.read(buf);
  assertEquals(res2, 3);
  assertEquals(decode(buf), "def");
  const res3 = await r.read(buf);
  assertEquals(res3, Deno.EOF);
  assertEquals(decode(buf), "def");
});

test(async function ioMultiReader(): Promise<void> {
  const r = new MultiReader(new StringReader("abc"), new StringReader("def"));
  const w = new StringWriter();
  const n = await copyN(w, r, 4);
  assertEquals(n, 4);
  assertEquals(w.toString(), "abcd");
  await copy(w, r);
  assertEquals(w.toString(), "abcdef");
});
