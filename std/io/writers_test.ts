// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import { assertEquals } from "../testing/asserts.ts";
import { StringWriter } from "./writers.ts";
import { StringReader } from "./readers.ts";
import { copyN } from "./ioutil.ts";

Deno.test("ioStringWriter", async function (): Promise<void> {
  const w = new StringWriter("base");
  const r = new StringReader("0123456789");
  await copyN(r, w, 4);
  assertEquals(w.toString(), "base0123");
  await Deno.copy(r, w);
  assertEquals(w.toString(), "base0123456789");
});

Deno.test("ioStringWriterSync", function (): void {
  const encoder = new TextEncoder();
  const w = new StringWriter("");
  w.writeSync(encoder.encode("deno"));
  assertEquals(w.toString(), "deno");
  w.writeSync(encoder.encode("\nland"));
  assertEquals(w.toString(), "deno\nland");
});
