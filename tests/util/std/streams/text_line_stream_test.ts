// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { TextLineStream } from "./text_line_stream.ts";
import { assertEquals } from "../assert/mod.ts";

Deno.test("TextLineStream() parses simple input", async () => {
  const stream = ReadableStream.from([
    "qwertzu",
    "iopasd\r\nmnbvc",
    "xylk\rjhgfds\napoiuzt\r",
    "qwr\r09ei\rqwrjiowqr\r",
    "\nrewq0987\n\n654321",
    "\nrewq0987\r\n\r\n654321\r",
  ]).pipeThrough(new TextLineStream());

  assertEquals(await Array.fromAsync(stream), [
    "qwertzuiopasd",
    "mnbvcxylk\rjhgfds",
    "apoiuzt\rqwr\r09ei\rqwrjiowqr",
    "rewq0987",
    "",
    "654321",
    "rewq0987",
    "",
    "654321\r",
  ]);

  const stream2 = ReadableStream.from("rewq0987\r\n\r\n654321\n")
    .pipeThrough(new TextLineStream());

  assertEquals(await Array.fromAsync(stream2), [
    "rewq0987",
    "",
    "654321",
  ]);
});

Deno.test("TextLineStream() parses with `allowCR` enabled", async () => {
  const stream = ReadableStream.from([
    "qwertzu",
    "iopasd\r\nmnbvc",
    "xylk\rjhgfds\napoiuzt\r",
    "qwr\r09ei\rqwrjiowqr\r",
    "\nrewq0987\n\n654321",
    "\nrewq0987\r\n\r\n654321\r",
  ]).pipeThrough(new TextLineStream({ allowCR: true }));
  assertEquals(await Array.fromAsync(stream), [
    "qwertzuiopasd",
    "mnbvcxylk",
    "jhgfds",
    "apoiuzt",
    "qwr",
    "09ei",
    "qwrjiowqr",
    "rewq0987",
    "",
    "654321",
    "rewq0987",
    "",
    "654321",
  ]);

  const stream2 = ReadableStream.from("rewq0987\r\n\r\n654321\n")
    .pipeThrough(new TextLineStream());

  assertEquals(await Array.fromAsync(stream2), [
    "rewq0987",
    "",
    "654321",
  ]);
});

Deno.test("TextLineStream() parses large chunks", async () => {
  const totalLines = 20_000;
  const stream = ReadableStream.from("\n".repeat(totalLines))
    .pipeThrough(new TextLineStream());
  const lines = await Array.fromAsync(stream);

  assertEquals(lines.length, totalLines);
  assertEquals(lines, Array.from({ length: totalLines }).fill(""));
});

Deno.test("TextLineStream() parses no final empty chunk with terminal newline", async () => {
  const stream = ReadableStream.from([
    "abc\n",
    "def\nghi\njk",
    "l\nmn",
    "o\np",
    "qr",
    "\nstu\nvwx\n",
    "yz\n",
  ]).pipeThrough(new TextLineStream());

  assertEquals(await Array.fromAsync(stream), [
    "abc",
    "def",
    "ghi",
    "jkl",
    "mno",
    "pqr",
    "stu",
    "vwx",
    "yz",
  ]);
});

Deno.test("TextLineStream() parses no final empty chunk without terminal newline", async () => {
  const stream = ReadableStream.from([
    "abc\n",
    "def\nghi\njk",
    "l\nmn",
    "o\np",
    "qr",
    "\nstu\nvwx\n",
    "yz",
  ]).pipeThrough(new TextLineStream());

  assertEquals(await Array.fromAsync(stream), [
    "abc",
    "def",
    "ghi",
    "jkl",
    "mno",
    "pqr",
    "stu",
    "vwx",
    "yz",
  ]);
});
