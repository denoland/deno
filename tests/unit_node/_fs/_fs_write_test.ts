// Copyright 2018-2025 the Deno authors. MIT license.

import { closeSync, openSync, write, writeSync } from "node:fs";
import { open } from "node:fs/promises";
import { assertEquals } from "@std/assert";
import { Buffer } from "node:buffer";

const decoder = new TextDecoder("utf-8");

Deno.test({
  name: "Data is written to the file with the correct length",
  async fn() {
    const tempFile: string = await Deno.makeTempFile();
    await using file = await open(tempFile, "w+");
    const buffer = Buffer.from("hello world");
    const bytesWrite = await new Promise((resolve, reject) => {
      write(file.fd, buffer, 0, 5, (err: unknown, nwritten: number) => {
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
  fn() {
    const tempFile: string = Deno.makeTempFileSync();
    const fd = openSync(tempFile, "w+");
    const buffer = Buffer.from("hello world");
    const bytesWrite = writeSync(fd, buffer, 0, 5);

    const data = Deno.readFileSync(tempFile);
    closeSync(fd);
    Deno.removeSync(tempFile);

    assertEquals(bytesWrite, 5);
    assertEquals(decoder.decode(data), "hello");
  },
});

Deno.test({
  name: "Async write with option object",
  async fn() {
    const tempFile = await Deno.makeTempFile();
    await using file = await open(tempFile, "w+");
    const buffer = Buffer.from("hello world!");

    // Write 'hello'
    const bytesWritten = await new Promise((resolve, reject) => {
      write(
        file.fd,
        buffer,
        { offset: 0, length: 5, position: 0 },
        (err: unknown, nwritten: number) => {
          if (err) return reject(err);
          resolve(nwritten);
        },
      );
    });

    const data = await Deno.readFile(tempFile);
    assertEquals(bytesWritten, 5);
    assertEquals(decoder.decode(data), "hello");

    // Write 'wo' at position 2
    const bytesWritten2 = await new Promise((resolve, reject) => {
      write(
        file.fd,
        buffer,
        { offset: 6, length: 2, position: 2 },
        (err: unknown, nwritten: number) => {
          if (err) return reject(err);
          resolve(nwritten);
        },
      );
    });

    const data2 = await Deno.readFile(tempFile);
    await Deno.remove(tempFile);
    assertEquals(bytesWritten2, 2);
    assertEquals(decoder.decode(data2), "hewoo");
  },
});

Deno.test({
  name: "Sync write with option object",
  fn() {
    const tempFile = Deno.makeTempFileSync();
    const fd = openSync(tempFile, "w+");
    const buffer = Buffer.from("hello world!");

    // Write 'hello'
    // TODO(Tango992): Delete the ts-expect-error when the @types/node definition has been defined.
    // @ts-expect-error option object is a valid `writeSync` argument
    const bytesWritten = writeSync(fd, buffer, {
      offset: 0,
      length: 5,
      position: 0,
    });

    const data = Deno.readFileSync(tempFile);
    assertEquals(bytesWritten, 5);
    assertEquals(decoder.decode(data), "hello");

    // Write 'wo' at position 2
    // TODO(Tango992): Delete the ts-expect-error when the @types/node definition has been defined.
    // @ts-expect-error option object is a valid `writeSync` argument
    const bytesWritten2 = writeSync(fd, buffer, {
      offset: 6,
      length: 2,
      position: 2,
    });

    const data2 = Deno.readFileSync(tempFile);
    Deno.removeSync(tempFile);
    assertEquals(bytesWritten2, 2);
    assertEquals(decoder.decode(data2), "hewoo");
    closeSync(fd);
  },
});

Deno.test({
  name: "Data is padded if position > length",
  fn() {
    const tempFile: string = Deno.makeTempFileSync();
    const fd = openSync(tempFile, "w+");

    const str = "hello world";
    const buffer = Buffer.from(str);
    const bytesWritten = writeSync(fd, buffer, 0, str.length, 4);

    const data = Deno.readFileSync(tempFile);
    closeSync(fd);
    Deno.removeSync(tempFile);

    assertEquals(bytesWritten, str.length);
    // Check if result is padded
    assertEquals(decoder.decode(data), "\x00\x00\x00\x00hello world");
  },
});

Deno.test({
  name: "write with offset TypedArray buffers",
  async fn() {
    const tempFile: string = Deno.makeTempFileSync();
    await using file = await open(tempFile, "w+");
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
          const buffer = new (constr as Uint8ArrayConstructor)(
            arrayBuffer,
            innerOffset,
            offset + bytes.length,
          );
          for (let i = 0; i < bytes.length; i++) {
            buffer[offset + i] = i;
          }
          let nWritten = writeSync(file.fd, buffer, offset, bytes.length, 0);

          let data = Deno.readFileSync(tempFile);

          assertEquals(nWritten, bytes.length);
          // console.log(constr, innerOffset, offset);
          assertEquals(data, new Uint8Array(bytes));
          nWritten = await new Promise((resolve, reject) =>
            write(
              file.fd,
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

Deno.test({
  name: "writeSync: negative position value writes at current position",
  fn() {
    const tempFile: string = Deno.makeTempFileSync();
    const fd = openSync(tempFile, "w+");
    const buffer = Buffer.from("hello world");

    // Write 'hello'
    writeSync(fd, buffer, 0, 5, -1);
    const data = Deno.readFileSync(tempFile);
    assertEquals(decoder.decode(data), "hello");

    // Write ' world'
    writeSync(fd, buffer, 5, 6, -1);
    const data2 = Deno.readFileSync(tempFile);
    assertEquals(decoder.decode(data2), "hello world");

    Deno.removeSync(tempFile);
    closeSync(fd);
  },
});

Deno.test({
  name: "write: negative position value writes at current position",
  async fn() {
    const tempFile: string = await Deno.makeTempFile();
    await using file = await open(tempFile, "w+");
    const buffer = Buffer.from("hello world");

    // Write 'hello'
    await new Promise((resolve, reject) => {
      write(file.fd, buffer, 0, 5, -1, (err) => {
        if (err) return reject(err);
        resolve(undefined);
      });
    });

    const data = await Deno.readFile(tempFile);
    assertEquals(decoder.decode(data), "hello");

    // Write ' world'
    await new Promise((resolve, reject) => {
      write(file.fd, buffer, 5, 6, -1, (err) => {
        if (err) return reject(err);
        resolve(undefined);
      });
    });

    const data2 = await Deno.readFile(tempFile);
    assertEquals(decoder.decode(data2), "hello world");

    await Deno.remove(tempFile);
  },
});
