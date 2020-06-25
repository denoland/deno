// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import {
  unitTest,
  assert,
  assertEquals,
  assertThrows,
  assertThrowsAsync,
  pathToAbsoluteFileUrl,
} from "./test_util.ts";

unitTest({ perms: { read: true } }, function readFileSyncSuccess(): void {
  const data = Deno.readFileSync("cli/tests/fixture.json");
  assert(data.byteLength > 0);
  const decoder = new TextDecoder("utf-8");
  const json = decoder.decode(data);
  const pkg = JSON.parse(json);
  assertEquals(pkg.name, "deno");
});

unitTest({ perms: { read: true } }, function readFileSyncUrl(): void {
  const data = Deno.readFileSync(
    pathToAbsoluteFileUrl("cli/tests/fixture.json")
  );
  assert(data.byteLength > 0);
  const decoder = new TextDecoder("utf-8");
  const json = decoder.decode(data);
  const pkg = JSON.parse(json);
  assertEquals(pkg.name, "deno");
});

unitTest({ perms: { read: false } }, function readFileSyncPerm(): void {
  assertThrows(() => {
    Deno.readFileSync("cli/tests/fixture.json");
  }, Deno.errors.PermissionDenied);
});

unitTest({ perms: { read: true } }, function readFileSyncNotFound(): void {
  assertThrows(() => {
    Deno.readFileSync("bad_filename");
  }, Deno.errors.NotFound);
});

unitTest({ perms: { read: true } }, async function readFileUrl(): Promise<
  void
> {
  const data = await Deno.readFile(
    pathToAbsoluteFileUrl("cli/tests/fixture.json")
  );
  assert(data.byteLength > 0);
  const decoder = new TextDecoder("utf-8");
  const json = decoder.decode(data);
  const pkg = JSON.parse(json);
  assertEquals(pkg.name, "deno");
});

unitTest({ perms: { read: true } }, async function readFileSuccess(): Promise<
  void
> {
  const data = await Deno.readFile("cli/tests/fixture.json");
  assert(data.byteLength > 0);
  const decoder = new TextDecoder("utf-8");
  const json = decoder.decode(data);
  const pkg = JSON.parse(json);
  assertEquals(pkg.name, "deno");
});

unitTest({ perms: { read: false } }, async function readFilePerm(): Promise<
  void
> {
  await assertThrowsAsync(async () => {
    await Deno.readFile("cli/tests/fixture.json");
  }, Deno.errors.PermissionDenied);
});

unitTest({ perms: { read: true } }, function readFileSyncLoop(): void {
  for (let i = 0; i < 256; i++) {
    Deno.readFileSync("cli/tests/fixture.json");
  }
});
