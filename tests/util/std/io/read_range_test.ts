// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { copy } from "../bytes/copy.ts";
import {
  assert,
  assertEquals,
  assertRejects,
  assertThrows,
} from "../assert/mod.ts";
import { readRange, readRangeSync } from "./read_range.ts";
import type { Closer, Reader, ReaderSync } from "../types.d.ts";

// N controls how many iterations of certain checks are performed.
const N = 100;
let testBytes: Uint8Array | undefined;

export function init() {
  if (testBytes === undefined) {
    testBytes = new Uint8Array(N);
    for (let i = 0; i < N; i++) {
      testBytes[i] = "a".charCodeAt(0) + (i % 26);
    }
  }
}

class MockFile
  implements Deno.Seeker, Deno.SeekerSync, Reader, ReaderSync, Closer {
  #buf: Uint8Array;
  #closed = false;
  #offset = 0;

  get closed() {
    return this.#closed;
  }

  constructor(buf: Uint8Array) {
    this.#buf = buf;
  }

  close() {
    this.#closed = true;
  }

  read(p: Uint8Array): Promise<number | null> {
    if (this.#offset >= this.#buf.length) {
      return Promise.resolve(null);
    }
    const nread = Math.min(p.length, 16_384, this.#buf.length - this.#offset);
    if (nread === 0) {
      return Promise.resolve(0);
    }
    copy(this.#buf.subarray(this.#offset, this.#offset + nread), p);
    this.#offset += nread;
    return Promise.resolve(nread);
  }

  readSync(p: Uint8Array): number | null {
    if (this.#offset >= this.#buf.length) {
      return null;
    }
    const nread = Math.min(p.length, 16_384, this.#buf.length - this.#offset);
    if (nread === 0) {
      return 0;
    }
    copy(this.#buf.subarray(this.#offset, this.#offset + nread), p);
    this.#offset += nread;
    return nread;
  }

  seek(offset: number, whence: Deno.SeekMode): Promise<number> {
    assert(whence === Deno.SeekMode.Start);
    if (offset >= this.#buf.length) {
      return Promise.reject(new RangeError("attempted to seek past end"));
    }
    this.#offset = offset;
    return Promise.resolve(this.#offset);
  }

  seekSync(offset: number, whence: Deno.SeekMode): number {
    assert(whence === Deno.SeekMode.Start);
    if (offset >= this.#buf.length) {
      throw new RangeError("attempted to seek past end");
    }
    this.#offset = offset;
    return this.#offset;
  }
}

Deno.test({
  name: "readRange",
  async fn() {
    init();
    assert(testBytes);
    const file = new MockFile(testBytes);
    const actual = await readRange(file, { start: 0, end: 9 });
    assertEquals(actual, testBytes.subarray(0, 10));
  },
});

Deno.test({
  name: "readRange - invalid range",
  async fn() {
    init();
    assert(testBytes);
    const file = new MockFile(testBytes);
    await assertRejects(
      async () => {
        await readRange(file, { start: 100, end: 0 });
      },
      Error,
      "Invalid byte range was passed.",
    );
  },
});

Deno.test({
  name: "readRange - read past EOF",
  async fn() {
    init();
    assert(testBytes);
    const file = new MockFile(testBytes);
    await assertRejects(
      async () => {
        await readRange(file, { start: 99, end: 100 });
      },
      Error,
      "Unexpected EOF reach while reading a range.",
    );
  },
});

Deno.test({
  name: "readRangeSync",
  fn() {
    init();
    assert(testBytes);
    const file = new MockFile(testBytes);
    const actual = readRangeSync(file, { start: 0, end: 9 });
    assertEquals(actual, testBytes.subarray(0, 10));
  },
});

Deno.test({
  name: "readRangeSync - invalid range",
  fn() {
    init();
    assert(testBytes);
    const file = new MockFile(testBytes);
    assertThrows(
      () => {
        readRangeSync(file, { start: 100, end: 0 });
      },
      Error,
      "Invalid byte range was passed.",
    );
  },
});

Deno.test({
  name: "readRangeSync - read past EOF",
  fn() {
    init();
    assert(testBytes);
    const file = new MockFile(testBytes);
    assertThrows(
      () => {
        readRangeSync(file, { start: 99, end: 100 });
      },
      Error,
      "Unexpected EOF reach while reading a range.",
    );
  },
});
