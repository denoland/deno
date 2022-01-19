
import { assertEquals } from "https://deno.land/std@0.121.0/testing/asserts.ts";
import * as pako from "https://deno.land/x/pako@v2.0.3/pako.js";


// This test asserts that compressing '' doesn't affect the compressed data.
// Example: compressing ['Hello', '', 'Hello'] results in 'HelloHello'

async function compressChunkList(chunkList, format) {
  const cs = new CompressionStream(format);
  const writer = cs.writable.getWriter();
  for (const chunk of chunkList) {
    const chunkByte = new TextEncoder().encode(chunk);
    writer.write(chunkByte);
  }
  const closePromise = writer.close();
  const out = [];
  const reader = cs.readable.getReader();
  let totalSize = 0;
  while (true) {
    const { value, done } = await reader.read();
    if (done)
      break;
    out.push(value);
    totalSize += value.byteLength;
  }
  await closePromise;
  const concatenated = new Uint8Array(totalSize);
  let offset = 0;
  for (const array of out) {
    concatenated.set(array, offset);
    offset += array.byteLength;
  }
  return concatenated;
}

const chunkLists = [
  ['', 'Hello', 'Hello'],
  ['Hello', '', 'Hello'],
  ['Hello', 'Hello', '']
];
const expectedValue = new TextEncoder().encode('HelloHello');

for (const chunkList of chunkLists) {
    const compressedData = await compressChunkList(chunkList, 'deflate');
    console.log("compressedData", compressedData);
    // decompress with pako, and check that we got the same result as our original string
    console.log("expectedValue", expectedValue);
    const actual = pako.inflate(compressedData);
    console.log("actual", actual);
    assertEquals(expectedValue, actual, 'value should match');

  /*
    const compressedData = await compressChunkList(chunkList, 'gzip');
    // decompress with pako, and check that we got the same result as our original string
    assertEquals(expectedValue, pako.inflate(compressedData), 'value should match');
  */
}
