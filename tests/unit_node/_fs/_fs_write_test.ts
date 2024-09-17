// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

// deno-lint-ignore-file no-console

import { write, writeSync } from "node:fs";
import { assertEquals } from "@std/assert";
import { Buffer } from "node:buffer";

const decoder = new TextDecoder("utf-8");

Deno.test({
  name: "Data is written to the file with the correct length",
  // TODO(bartlomieju): this test is broken in Deno 2, because `file.rid` is undefined.
  // The fs APIs should be rewritten to use actual FDs, not RIDs
  ignore: true,
  async fn() {
    const tempFile: string = await Deno.makeTempFile();
    using file = await Deno.open(tempFile, {
      create: true,
      write: true,
      read: true,
    });
    const buffer = Buffer.from("hello world");
    const bytesWrite = await new Promise((resolve, reject) => {
      // @ts-ignore (iuioiua) `file.rid` should no longer be needed once FDs are used
      write(file.rid, buffer, 0, 5, (err: unknown, nwritten: number) => {
        if (err) return reject(err);
        resolve(nwritten);
      });
    });

    const data = await Deno.readFile(tempFile);
    await Deno.remove(tempFile);

    assertEquals(bytesWrite, 5);
    assertEquals(decoder.decode(data), "hello");
  },
});

Deno.test({
  name: "Data is written synchronously to the file with the correct length",
  // TODO(bartlomieju): this test is broken in Deno 2, because `file.rid` is undefined.
  // The fs APIs should be rewritten to use actual FDs, not RIDs
  ignore: true,
  fn() {
    const tempFile: string = Deno.makeTempFileSync();
    using file = Deno.openSync(tempFile, {
      create: true,
      write: true,
      read: true,
    });
    const buffer = Buffer.from("hello world");
    // @ts-ignore (iuioiua) `file.rid` should no longer be needed once FDs are used
    const bytesWrite = writeSync(file.rid, buffer, 0, 5);

    const data = Deno.readFileSync(tempFile);
    Deno.removeSync(tempFile);

    assertEquals(bytesWrite, 5);
    assertEquals(decoder.decode(data), "hello");
  },
});

Deno.test({
  name: "Data is padded if position > length",
  // TODO(bartlomieju): this test is broken in Deno 2, because `file.rid` is undefined.
  // The fs APIs should be rewritten to use actual FDs, not RIDs
  ignore: true,
  async fn() {
    const tempFile: string = Deno.makeTempFileSync();

    using file = await Deno.open(tempFile, {
      create: true,
      write: true,
      read: true,
    });

    const str = "hello world";
    const buffer = Buffer.from(str);
    // @ts-ignore (iuioiua) `file.rid` should no longer be needed once FDs are used
    const bytesWritten = writeSync(file.rid, buffer, 0, str.length, 4);

    const data = Deno.readFileSync(tempFile);
    Deno.removeSync(tempFile);

    assertEquals(bytesWritten, str.length);
    // Check if result is padded
    assertEquals(decoder.decode(data), "\x00\x00\x00\x00hello world");
  },
});

Deno.test({
  name: "write with offset TypedArray buffers",
  // TODO(bartlomieju): this test is broken in Deno 2, because `file.rid` is undefined.
  // The fs APIs should be rewritten to use actual FDs, not RIDs
  ignore: true,
  async fn() {
    const tempFile: string = Deno.makeTempFileSync();
    using file = Deno.openSync(tempFile, {
      create: true,
      write: true,
      read: true,
    });
    const arrayBuffer = new ArrayBuffer(128);
    const resetBuffer = () => {
      new Uint8Array(arrayBuffer).fill(0);
    };
    const bufConstructors = [
      Int8Array,
      Uint8Array,
    ];
    const offsets = [0, 24, 48];
    const bytes = [0, 1, 2, 3, 4];
    for (const constr of bufConstructors) {
      // test combinations of buffers internally offset from their backing array buffer,
      // and also offset in the write call
      for (const innerOffset of offsets) {
        for (const offset of offsets) {
          resetBuffer();
          const buffer = new constr(
            arrayBuffer,
            innerOffset,
            offset + bytes.length,
          );
          for (let i = 0; i < bytes.length; i++) {
            buffer[offset + i] = i;
          }
          // @ts-ignore (iuioiua) `file.rid` should no longer be needed once FDs are used
          let nWritten = writeSync(file.rid, buffer, offset, bytes.length, 0);

          let data = Deno.readFileSync(tempFile);

          assertEquals(nWritten, bytes.length);
          console.log(constr, innerOffset, offset);
          assertEquals(data, new Uint8Array(bytes));
          nWritten = await new Promise((resolve, reject) =>
            write(
              // @ts-ignore (iuioiua) `file.rid` should no longer be needed once FDs are used
              file.rid,
              buffer,
              offset,
              bytes.length,
              0,
              (err: unknown, nwritten: number) => {
                if (err) return reject(err);
                resolve(nwritten);
              },
            )
          );

          data = Deno.readFileSync(tempFile);
          assertEquals(nWritten, 5);
          assertEquals(data, new Uint8Array(bytes));
        }
      }
    }
  },
});
