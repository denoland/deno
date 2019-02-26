const { copy } = Deno;
import { assert, test } from "../testing/mod.ts";
import { MultiReader, StringReader } from "./readers.ts";
import { StringWriter } from "./writers.ts";
import { copyN } from "./ioutil.ts";
import { decode } from "../strings/strings.ts";

test(async function ioStringReader() {
  const r = new StringReader("abcdef");
  const { nread, eof } = await r.read(new Uint8Array(6));
  assert.equal(nread, 6);
  assert.equal(eof, true);
});

test(async function ioStringReader() {
  const r = new StringReader("abcdef");
  const buf = new Uint8Array(3);
  let res1 = await r.read(buf);
  assert.equal(res1.nread, 3);
  assert.equal(res1.eof, false);
  assert.equal(decode(buf), "abc");
  let res2 = await r.read(buf);
  assert.equal(res2.nread, 3);
  assert.equal(res2.eof, true);
  assert.equal(decode(buf), "def");
});

test(async function ioMultiReader() {
  const r = new MultiReader(new StringReader("abc"), new StringReader("def"));
  const w = new StringWriter();
  const n = await copyN(w, r, 4);
  assert.equal(n, 4);
  assert.equal(w.toString(), "abcd");
  await copy(w, r);
  assert.equal(w.toString(), "abcdef");
});
