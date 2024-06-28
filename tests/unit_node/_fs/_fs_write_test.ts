// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
import { write, writeSync } from "node:fs";
import { assertEquals } from "@std/assert/mod.ts";
import { Buffer } from "node:buffer";

const decoder = new TextDecoder("utf-8");

Deno.test({
  name: "Data is written to the file with the correct length",
  async fn() {
    const tempFile: string = await Deno.makeTempFile();
    using file = await Deno.open(tempFile, {
      create: true,
      write: true,
      read: true,
    });
    const buffer = Buffer.from("hello world");
    const bytesWrite = await new Promise((resolve, reject) => {
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
  fn() {
    const tempFile: string = Deno.makeTempFileSync();
    using file = Deno.openSync(tempFile, {
      create: true,
      write: true,
      read: true,
    });
    const buffer = Buffer.from("hello world");
    const bytesWrite = writeSync(file.rid, buffer, 0, 5);

    const data = Deno.readFileSync(tempFile);
    Deno.removeSync(tempFile);

    assertEquals(bytesWrite, 5);
    assertEquals(decoder.decode(data), "hello");
  },
});

Deno.test({
  name: "Data is padded if position > length",
  async fn() {
    const tempFile: string = Deno.makeTempFileSync();

    using file = await Deno.open(tempFile, {
      create: true,
      write: true,
      read: true,
    });

    const str = "hello world";
    const buffer = Buffer.from(str);
    const bytesWritten = writeSync(file.rid, buffer, 0, str.length, 4);

    const data = Deno.readFileSync(tempFile);
    Deno.removeSync(tempFile);

    assertEquals(bytesWritten, str.length);
    // Check if result is padded
    assertEquals(decoder.decode(data), "\x00\x00\x00\x00hello world");
  },
});
