const { copy } = Deno;
import { test } from "../testing/mod.ts";
import { assertEq } from "../testing/asserts.ts";
import { StringWriter } from "./writers.ts";
import { StringReader } from "./readers.ts";
import { copyN } from "./ioutil.ts";

test(async function ioStringWriter() {
  const w = new StringWriter("base");
  const r = new StringReader("0123456789");
  await copyN(w, r, 4);
  assertEq(w.toString(), "base0123");
  await copy(w, r);
  assertEq(w.toString(), "base0123456789");
});
