import { assertEquals } from "./test_util/std/testing/asserts.ts";

Deno.test(async function foo() {
  async function readAllChunks(stream) {
    let out = [];
    let i = 0;
    const reader = stream.getReader({ mode: "byob" });
    let readValue = await reader.read(new Uint8Array(64));

    while (!readValue.done) {
      for (const val of readValue.value) {
        out[i++] = val;
      }
      readValue = await reader.read(new Uint8Array(64));
    }

    return out;
  }

  const inputArr = [8, 241, 48, 123, 151];
  const typedArr = new Uint8Array(inputArr);
  const blob = new Blob([typedArr]);
  const stream = blob.stream();
  const out = await readAllChunks(stream);
  assertEquals(out, inputArr);
});

Deno.test(async function bar() {
  async function readAllChunks(stream) {
    let out = [];
    let i = 0;
    const reader = stream.getReader();
    let readValue = await reader.read();

    while (!readValue.done) {
      for (const val of readValue.value) {
        out[i++] = val;
      }
      readValue = await reader.read(new Uint8Array(64));
    }

    return out;
  }

  const inputArr = [8, 241, 48, 123, 151];
  const typedArr = new Uint8Array(inputArr);
  const blob = new Blob([typedArr]);
  const stream = blob.stream();
  const out = await readAllChunks(stream);
  assertEquals(out, inputArr);
});
