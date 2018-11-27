import { Buffer, readAll } from "deno";
import { assertEqual, test } from "./test_util.ts";

const N = 100;

function createBytes() {
  const bytes = [];
  for (let i = 0; i < N; ++i) {
    bytes.push("a".charCodeAt(0) + (i % 26));
  }
  return new Uint8Array(bytes);
}

test(async function testReadAll() {
  const testBytes = createBytes();
  const reader = new Buffer(testBytes.buffer as ArrayBuffer);
  const actualBytes = await readAll(reader);
  assertEqual(testBytes.byteLength, actualBytes.byteLength);
  for (let i = 0; i < testBytes.length; ++i) {
    assertEqual(testBytes[i], actualBytes[i]);
  }
});
