const { copy, test } = Deno;
import { assertEquals } from "../testing/asserts.ts";
import {
  MultiReader,
  StringReader,
  bytesReader,
  emptyReader,
} from "./readers.ts";
import { StringWriter } from "./writers.ts";
import { copyN } from "./ioutil.ts";
import { decode } from "../encoding/utf8.ts";

test("ioStringReader", async function (): Promise<void> {
  const r = new StringReader("abcdef");
  const res0 = await r.read(new Uint8Array(6));
  assertEquals(res0, 6);
  const res1 = await r.read(new Uint8Array(6));
  assertEquals(res1, null);
});

test("ioStringReader", async function (): Promise<void> {
  const r = new StringReader("abcdef");
  const buf = new Uint8Array(3);
  const res1 = await r.read(buf);
  assertEquals(res1, 3);
  assertEquals(decode(buf), "abc");
  const res2 = await r.read(buf);
  assertEquals(res2, 3);
  assertEquals(decode(buf), "def");
  const res3 = await r.read(buf);
  assertEquals(res3, null);
  assertEquals(decode(buf), "def");
});

test("ioMultiReader", async function (): Promise<void> {
  const r = new MultiReader(new StringReader("abc"), new StringReader("def"));
  const w = new StringWriter();
  const n = await copyN(r, w, 4);
  assertEquals(n, 4);
  assertEquals(w.toString(), "abcd");
  await copy(r, w);
  assertEquals(w.toString(), "abcdef");
});

test({
  name: "[io/readers] bytesReader",
  async fn() {
    const bytes = new Uint8Array(Array.from({ length: 10 }).map((_, i) => i));
    const br = bytesReader(bytes);
    const buf = new Uint8Array(3);
    const dest = new Deno.Buffer();
    let r: number | null;
    while ((r = await br.read(buf)) != null) {
      await dest.write(buf.subarray(0, r));
    }
    assertEquals(dest.bytes(), bytes);
  },
});

test({
  name: "[io/readers] emptyReader",
  async fn() {
    const er = emptyReader();
    const buf = new Uint8Array();
    assertEquals(await er.read(buf), null);
    assertEquals(await er.read(buf), null);
  },
});
