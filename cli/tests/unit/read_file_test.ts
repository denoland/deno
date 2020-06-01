// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { unitTest, assert, assertEquals } from "./test_util.ts";
import { resolve } from "../../../std/path/mod.ts";

const getResolvedUrl = (path: string): URL =>
  new URL(
    Deno.build.os === "windows"
      ? "file:///" + resolve(path).replace(/\\/g, "/")
      : "file://" + resolve(path)
  );

unitTest({ perms: { read: true } }, function readFileSyncSuccess(): void {
  const data = Deno.readFileSync("cli/tests/fixture.json");
  assert(data.byteLength > 0);
  const decoder = new TextDecoder("utf-8");
  const json = decoder.decode(data);
  const pkg = JSON.parse(json);
  assertEquals(pkg.name, "deno");
});

unitTest({ perms: { read: true } }, function readFileSyncUrl(): void {
  const data = Deno.readFileSync(getResolvedUrl("cli/tests/fixture.json"));
  assert(data.byteLength > 0);
  const decoder = new TextDecoder("utf-8");
  const json = decoder.decode(data);
  const pkg = JSON.parse(json);
  assertEquals(pkg.name, "deno");
});

unitTest({ perms: { read: false } }, function readFileSyncPerm(): void {
  let caughtError = false;
  try {
    Deno.readFileSync("cli/tests/fixture.json");
  } catch (e) {
    caughtError = true;
    assert(e instanceof Deno.errors.PermissionDenied);
  }
  assert(caughtError);
});

unitTest({ perms: { read: true } }, function readFileSyncNotFound(): void {
  let caughtError = false;
  let data;
  try {
    data = Deno.readFileSync("bad_filename");
  } catch (e) {
    caughtError = true;
    assert(e instanceof Deno.errors.NotFound);
  }
  assert(caughtError);
  assert(data === undefined);
});

unitTest({ perms: { read: true } }, async function readFileUrl(): Promise<
  void
> {
  const data = await Deno.readFile(getResolvedUrl("cli/tests/fixture.json"));
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
  let caughtError = false;
  try {
    await Deno.readFile("cli/tests/fixture.json");
  } catch (e) {
    caughtError = true;
    assert(e instanceof Deno.errors.PermissionDenied);
  }
  assert(caughtError);
});

unitTest({ perms: { read: true } }, function readFileSyncLoop(): void {
  for (let i = 0; i < 256; i++) {
    Deno.readFileSync("cli/tests/fixture.json");
  }
});
