// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { CsvStringifyStream } from "./csv_stringify_stream.ts";
import { StringifyError } from "./stringify.ts";
import { assertEquals, assertRejects } from "../assert/mod.ts";

Deno.test({
  name: "[csv/csv_stringify_stream] CsvStringifyStream",
  permissions: "none",
  fn: async (t) => {
    await t.step("with arrays", async () => {
      const readable = ReadableStream.from([
        ["id", "name"],
        [1, "foo"],
        [2, "bar"],
      ]).pipeThrough(new CsvStringifyStream());
      const output = await Array.fromAsync(readable);
      assertEquals(output, [
        "id,name\r\n",
        "1,foo\r\n",
        "2,bar\r\n",
      ]);
    });

    await t.step("with arrays, columns", async () => {
      const readable = ReadableStream.from([
        [1, "foo"],
        [2, "bar"],
        // @ts-expect-error `columns` option is not allowed
      ]).pipeThrough(new CsvStringifyStream({ columns: ["id", "name"] }));
      await assertRejects(
        async () => await Array.fromAsync(readable),
        StringifyError,
      );
    });

    await t.step("with `separator`", async () => {
      const readable = ReadableStream.from([
        [1, "one"],
        [2, "two"],
        [3, "three"],
      ]).pipeThrough(new CsvStringifyStream({ separator: "\t" }));
      const output = await Array.fromAsync(readable);
      assertEquals(output, [
        "1\tone\r\n",
        "2\ttwo\r\n",
        "3\tthree\r\n",
      ]);
    });

    await t.step("with invalid `separator`", async () => {
      const readable = ReadableStream.from([
        ["one", "two", "three"],
      ]).pipeThrough(new CsvStringifyStream({ separator: "\r\n" }));
      await assertRejects(
        async () => await Array.fromAsync(readable),
        StringifyError,
      );
    });

    await t.step("with objects", async () => {
      const readable = ReadableStream.from([
        { id: 1, name: "foo" },
        { id: 2, name: "bar" },
        { id: 3, name: "baz" },
      ]).pipeThrough(new CsvStringifyStream({ columns: ["id", "name"] }));
      const output = await Array.fromAsync(readable);
      assertEquals(output, [
        "id,name\r\n",
        "1,foo\r\n",
        "2,bar\r\n",
        "3,baz\r\n",
      ]);
    });

    await t.step("with objects, no columns", async () => {
      const readable = ReadableStream.from([
        { id: 1, name: "foo" },
        { id: 2, name: "bar" },
        { id: 3, name: "baz" },
        // @ts-expect-error `columns` option is required
      ]).pipeThrough(new CsvStringifyStream());
      await assertRejects(
        async () => await Array.fromAsync(readable),
        StringifyError,
      );
    });
  },
});
