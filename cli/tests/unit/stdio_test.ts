// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { unitTest, assertEquals } from "./test_util.ts";

unitTest(async function stdioStdinRead() {
  const nread = await Deno.stdin.read(new Uint8Array(0));
  assertEquals(nread, 0);
});

unitTest(function stdioStdinReadSync() {
  const nread = Deno.stdin.readSync(new Uint8Array(0));
  assertEquals(nread, 0);
});

unitTest(async function stdioStdoutWrite() {
  const nwritten = await Deno.stdout.write(new Uint8Array(0));
  assertEquals(nwritten, 0);
});

unitTest(function stdioStdoutWriteSync() {
  const nwritten = Deno.stdout.writeSync(new Uint8Array(0));
  assertEquals(nwritten, 0);
});

unitTest(async function stdioStderrWrite() {
  const nwritten = await Deno.stderr.write(new Uint8Array(0));
  assertEquals(nwritten, 0);
});

unitTest(function stdioStderrWriteSync() {
  const nwritten = Deno.stderr.writeSync(new Uint8Array(0));
  assertEquals(nwritten, 0);
});
