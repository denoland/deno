const { copy, test } = Deno;
import { assertEquals } from "../testing/asserts.ts";
import { StringWriter } from "./writers.ts";
import { StringReader } from "./readers.ts";
import { copyN } from "./ioutil.ts";

test("ioStringWriter", async function (): Promise<void> {
  const w = new StringWriter("base");
  const r = new StringReader("0123456789");
  await copyN(r, w, 4);
  assertEquals(w.toString(), "base0123");
  await copy(r, w);
  assertEquals(w.toString(), "base0123456789");
});
