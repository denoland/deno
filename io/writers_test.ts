const { copy } = Deno;
import { test } from "../testing/mod.ts";
import { assertEquals } from "../testing/asserts.ts";
import { StringWriter } from "./writers.ts";
import { StringReader } from "./readers.ts";
import { copyN } from "./ioutil.ts";

test(async function ioStringWriter() {
  const w = new StringWriter("base");
  const r = new StringReader("0123456789");
  await copyN(w, r, 4);
  assertEquals(w.toString(), "base0123");
  await copy(w, r);
  assertEquals(w.toString(), "base0123456789");
});
