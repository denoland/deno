import { assert, runTests, test } from "../testing/mod.ts";
import { ChunkedBodyReader } from "./readers.ts";
import { StringReader } from "../io/readers.ts";
import { Buffer, copy } from "deno";

test(async function httpChunkedBodyReader() {
  const chunked = "3\r\nabc\r\n5\r\ndefgh\r\n0\r\n\r\n";
  const r = new ChunkedBodyReader(new StringReader(chunked));
  const w = new Buffer();
  await copy(w, r);
  assert.equal(w.toString(), "abcdefgh");
});
