// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import { assertEquals } from "./test_util.ts";

Deno.test("stdioStdinRead", async function () {
  const nread = await Deno.stdin.read(new Uint8Array(0));
  assertEquals(nread, 0);
});

Deno.test("stdioStdinReadSync", function () {
  const nread = Deno.stdin.readSync(new Uint8Array(0));
  assertEquals(nread, 0);
});

Deno.test("stdioStdoutWrite", async function () {
  const nwritten = await Deno.stdout.write(new Uint8Array(0));
  assertEquals(nwritten, 0);
});

Deno.test("stdioStdoutWriteSync", function () {
  const nwritten = Deno.stdout.writeSync(new Uint8Array(0));
  assertEquals(nwritten, 0);
});

Deno.test("stdioStderrWrite", async function () {
  const nwritten = await Deno.stderr.write(new Uint8Array(0));
  assertEquals(nwritten, 0);
});

Deno.test("stdioStderrWriteSync", function () {
  const nwritten = Deno.stderr.writeSync(new Uint8Array(0));
  assertEquals(nwritten, 0);
});
