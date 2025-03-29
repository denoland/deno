// Copyright 2018-2025 the Deno authors. MIT license.
/// <reference types="npm:@types/node" />
import {
  assert,
  assertEquals,
  assertFalse,
  assertMatch,
  assertStrictEquals,
} from "@std/assert";
import { read, readSync } from "node:fs";
import { open, openSync } from "node:fs";
import { Buffer } from "node:buffer";
import * as path from "@std/path";
import { closeSync } from "node:fs";

async function readTest<T extends NodeJS.ArrayBufferView>(
  testData: string,
  buffer: T,
  offset: number,
  length: number,
  position: number | null = null,
  expected: (
    fd: number,
    bytesRead: number | null,
    data: T | undefined,
  ) => void,
) {
  let fd1 = 0;
  await new Promise<{
    fd: number;
    bytesRead: number | null;
    data: T | undefined;
  }>((resolve, reject) => {
    open(testData, "r", (err, fd) => {
      if (err) reject(err);
      read(fd, buffer, offset, length, position, (err, bytesRead, data) => {
        if (err) reject(err);
        resolve({ fd, bytesRead, data });
      });
    });
  })
    .then(({ fd, bytesRead, data }) => {
      fd1 = fd;
      expected(fd, bytesRead, data);
    })
    .finally(() => closeSync(fd1));
}

Deno.test({
  name: "readSuccess",
  async fn() {
    const moduleDir = path.dirname(path.fromFileUrl(import.meta.url));
    const testData = path.resolve(moduleDir, "testdata", "hello.txt");
    const buf = Buffer.alloc(1024);
    await readTest(
      testData,
      buf,
      buf.byteOffset,
      buf.byteLength,
      null,
      (_fd, bytesRead, data) => {
        assertStrictEquals(bytesRead, 11);
        assertEquals(data instanceof Buffer, true);
        assertMatch((data as Buffer).toString(), /hello world/);
      },
    );
  },
});

Deno.test({
  name:
    "[std/node/fs] Read only five bytes, so that the position moves to five",
  async fn() {
    const moduleDir = path.dirname(path.fromFileUrl(import.meta.url));
    const testData = path.resolve(moduleDir, "testdata", "hello.txt");
    const buf = Buffer.alloc(5);
    await readTest(
      testData,
      buf,
      buf.byteOffset,
      5,
      null,
      (_fd, bytesRead, data) => {
        assertStrictEquals(bytesRead, 5);
        assertEquals(data instanceof Buffer, true);
        assertEquals((data as Buffer).toString(), "hello");
      },
    );
  },
});

Deno.test({
  name:
    "[std/node/fs] position option of fs.read() specifies where to begin reading from in the file",
  async fn() {
    const moduleDir = path.dirname(path.fromFileUrl(import.meta.url));
    const testData = path.resolve(moduleDir, "testdata", "hello.txt");
    const fd = openSync(testData, "r");
    const buf = Buffer.alloc(5);
    const positions = [6, 0, -1, null];
    const expected = [
      [119, 111, 114, 108, 100],
      [104, 101, 108, 108, 111],
      [104, 101, 108, 108, 111],
      [32, 119, 111, 114, 108],
    ];
    for (const [i, position] of positions.entries()) {
      await new Promise((resolve) => {
        read(
          fd,
          {
            buffer: buf,
            offset: buf.byteOffset,
            length: buf.byteLength,
            position,
          },
          (err, bytesRead, data) => {
            assertEquals(err, null);
            assertStrictEquals(bytesRead, 5);
            assertEquals(
              data,
              Buffer.from(expected[i]),
            );
            return resolve(true);
          },
        );
      });
    }
    closeSync(fd);
  },
});

Deno.test({
  name: "[std/node/fs] Read fs.read(fd, options, cb) signature",
  async fn() {
    const { promise, reject, resolve } = Promise.withResolvers<void>();
    const file = Deno.makeTempFileSync();
    Deno.writeTextFileSync(file, "hi there");
    const fd = openSync(file, "r+");
    const buf = Buffer.alloc(11);
    read(
      fd,
      {
        buffer: buf,
        offset: buf.byteOffset,
        length: buf.byteLength,
        position: null,
      },
      (err, bytesRead, data) => {
        try {
          assertEquals(err, null);
          assertStrictEquals(bytesRead, 8);
          assertEquals(
            data,
            Buffer.from([104, 105, 32, 116, 104, 101, 114, 101, 0, 0, 0]),
          );
        } catch (e) {
          reject(e);
          return;
        }
        resolve();
      },
    );
    closeSync(fd);
    await promise;
  },
});

Deno.test({
  name: "[std/node/fs] Read fs.read(fd, cb) signature",
  async fn() {
    const { promise, resolve, reject } = Promise.withResolvers<void>();
    const file = Deno.makeTempFileSync();
    Deno.writeTextFileSync(file, "hi deno");
    const fd = openSync(file, "r+");
    read(fd, (err, bytesRead, data) => {
      try {
        assertEquals(err, null);
        assertStrictEquals(bytesRead, 7);
        assertStrictEquals(data?.byteLength, 16384);
      } catch (e) {
        reject(e);
        return;
      }
      resolve();
    });
    closeSync(fd);
    await promise;
  },
});

Deno.test({
  name: "SYNC: readSuccess",
  fn() {
    const moduleDir = path.dirname(path.fromFileUrl(import.meta.url));
    const testData = path.resolve(moduleDir, "testdata", "hello.txt");
    const buffer = Buffer.alloc(1024);
    const fd = openSync(testData, "r");
    const bytesRead = readSync(
      fd,
      buffer,
      buffer.byteOffset,
      buffer.byteLength,
      null,
    );
    assertStrictEquals(bytesRead, 11);
    closeSync(fd);
  },
});

Deno.test({
  name: "[std/node/fs] Read only two bytes, so that the position moves to two",
  fn() {
    const moduleDir = path.dirname(path.fromFileUrl(import.meta.url));
    const testData = path.resolve(moduleDir, "testdata", "hello.txt");
    const buffer = Buffer.alloc(2);
    const fd = openSync(testData, "r");
    const bytesRead = readSync(fd, buffer, buffer.byteOffset, 2, null);
    assertStrictEquals(bytesRead, 2);
    closeSync(fd);
  },
});

Deno.test({
  name:
    "[std/node/fs] position option of fs.readSync() specifies where to begin reading from in the file",
  fn() {
    const moduleDir = path.dirname(path.fromFileUrl(import.meta.url));
    const testData = path.resolve(moduleDir, "testdata", "hello.txt");
    const fd = openSync(testData, "r");
    const buf = Buffer.alloc(5);
    const positions = [6, 0, -1, null];
    const expected = [
      [119, 111, 114, 108, 100],
      [104, 101, 108, 108, 111],
      [104, 101, 108, 108, 111],
      [32, 119, 111, 114, 108],
    ];
    for (const [i, position] of positions.entries()) {
      const bytesRead = readSync(
        fd,
        buf,
        buf.byteOffset,
        buf.byteLength,
        position,
      );
      assertStrictEquals(bytesRead, 5);
      assertEquals(
        buf,
        Buffer.from(expected[i]),
      );
    }
    closeSync(fd);
  },
});

Deno.test({
  name: "[std/node/fs] Read fs.readSync(fd, buffer[, options]) signature",
  fn() {
    const file = Deno.makeTempFileSync();
    Deno.writeTextFileSync(file, "hello deno");
    const buffer = Buffer.alloc(1024);
    const fd = openSync(file, "r+");
    const bytesRead = readSync(fd, buffer, {
      length: buffer.byteLength,
      offset: buffer.byteOffset,
      position: null,
    });
    assertStrictEquals(bytesRead, 10);
    closeSync(fd);
  },
});

Deno.test({
  name: "[std/node/fs] fs.read is async",
  async fn(t) {
    const file = await Deno.makeTempFile();
    await Deno.writeTextFile(file, "abc");

    await t.step("without position option", async () => {
      const { promise, resolve } = Promise.withResolvers<void>();
      let called = false;
      const fd = openSync(file, "r");
      read(fd, () => {
        called = true;
        closeSync(fd);
        resolve();
      });
      assertFalse(called);
      await promise;
    });

    await t.step("with position option", async () => {
      const { promise, resolve } = Promise.withResolvers<void>();
      let called = false;
      const buffer = Buffer.alloc(2);
      const fd = openSync(file, "r");
      read(fd, { position: 1, buffer, offset: 0, length: 2 }, () => {
        called = true;
        closeSync(fd);
        resolve();
      });
      assertFalse(called);
      await promise;
    });

    await Deno.remove(file);
  },
});

Deno.test({
  name: "SYNC: read with no offsetOropts argument",
  fn() {
    const moduleDir = path.dirname(path.fromFileUrl(import.meta.url));
    const testData = path.resolve(moduleDir, "testdata", "hello.txt");
    const buffer = Buffer.alloc(1024);
    const fd = openSync(testData, "r");
    const _bytesRead = readSync(
      fd,
      buffer,
    );
    closeSync(fd);
  },
});

Deno.test({
  name: "read with offset TypedArray buffers",
  async fn() {
    const moduleDir = path.dirname(path.fromFileUrl(import.meta.url));
    const testData = path.resolve(moduleDir, "testdata", "hello.txt");
    const buffer = new ArrayBuffer(1024);

    const bufConstructors = [
      Int8Array,
      Uint8Array,
    ];
    const offsets = [0, 24, 48];

    const resetBuffer = () => {
      new Uint8Array(buffer).fill(0);
    };
    const decoder = new TextDecoder();

    for (const constr of bufConstructors) {
      // test combinations of buffers internally offset from their backing array buffer,
      // and also offset in the read call
      for (const innerOffset of offsets) {
        for (const offset of offsets) {
          // test read
          resetBuffer();
          // deno-lint-ignore no-explicit-any
          const buf = new (constr as any)(
            buffer,
            innerOffset,
          ) as Int8Array | Uint8Array;
          await readTest(
            testData,
            buf,
            offset,
            buf.byteLength - offset,
            null,
            (_fd, bytesRead, data) => {
              assert(data);
              assert(bytesRead);
              assertStrictEquals(bytesRead, 11);
              assertEquals(data == buf, true);
              const got = decoder.decode(
                data.subarray(
                  offset,
                  offset + bytesRead,
                ),
              );
              const want = "hello world";
              assertEquals(got.length, want.length);
              assertEquals(
                got,
                want,
              );
            },
          );

          // test readSync
          resetBuffer();
          const fd = openSync(testData, "r");
          try {
            const bytesRead = readSync(
              fd,
              buf,
              offset,
              buf.byteLength - offset,
              null,
            );

            assertStrictEquals(bytesRead, 11);
            assertEquals(
              decoder.decode(buf.subarray(offset, offset + bytesRead)),
              "hello world",
            );
          } finally {
            closeSync(fd);
          }
        }
      }
    }
  },
});
