// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
import { assertEquals } from "./test_util.ts";

Deno.test(async function stdioStdinRead() {
  const nread = await Deno.stdin.read(new Uint8Array(0));
  assertEquals(nread, 0);
});

Deno.test(function stdioStdinReadSync() {
  const nread = Deno.stdin.readSync(new Uint8Array(0));
  assertEquals(nread, 0);
});

Deno.test(async function stdioStdoutWrite() {
  const nwritten = await Deno.stdout.write(new Uint8Array(0));
  assertEquals(nwritten, 0);
});

Deno.test(function stdioStdoutWriteSync() {
  const nwritten = Deno.stdout.writeSync(new Uint8Array(0));
  assertEquals(nwritten, 0);
});

Deno.test(async function stdioStderrWrite() {
  const nwritten = await Deno.stderr.write(new Uint8Array(0));
  assertEquals(nwritten, 0);
});

Deno.test(function stdioStderrWriteSync() {
  const nwritten = Deno.stderr.writeSync(new Uint8Array(0));
  assertEquals(nwritten, 0);
});
