// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import {
  assert,
  assertEquals,
  assertThrows,
  assertThrowsAsync,
  pathToAbsoluteFileUrl,
} from "./test_util.ts";

Deno.test("readFileSyncSuccess", function (): void {
  const data = Deno.readFileSync("cli/tests/fixture.json");
  assert(data.byteLength > 0);
  const decoder = new TextDecoder("utf-8");
  const json = decoder.decode(data);
  const pkg = JSON.parse(json);
  assertEquals(pkg.name, "deno");
});

Deno.test("readFileSyncUrl", function (): void {
  const data = Deno.readFileSync(
    pathToAbsoluteFileUrl("cli/tests/fixture.json"),
  );
  assert(data.byteLength > 0);
  const decoder = new TextDecoder("utf-8");
  const json = decoder.decode(data);
  const pkg = JSON.parse(json);
  assertEquals(pkg.name, "deno");
});

Deno.test("readFileSyncNotFound", function (): void {
  assertThrows(() => {
    Deno.readFileSync("bad_filename");
  }, Deno.errors.NotFound);
});

Deno.test("readFileUrl", async function (): Promise<
  void
> {
  const data = await Deno.readFile(
    pathToAbsoluteFileUrl("cli/tests/fixture.json"),
  );
  assert(data.byteLength > 0);
  const decoder = new TextDecoder("utf-8");
  const json = decoder.decode(data);
  const pkg = JSON.parse(json);
  assertEquals(pkg.name, "deno");
});

Deno.test("readFileSuccess", async function (): Promise<
  void
> {
  const data = await Deno.readFile("cli/tests/fixture.json");
  assert(data.byteLength > 0);
  const decoder = new TextDecoder("utf-8");
  const json = decoder.decode(data);
  const pkg = JSON.parse(json);
  assertEquals(pkg.name, "deno");
});

Deno.test("readFileSyncLoop", function (): void {
  for (let i = 0; i < 256; i++) {
    Deno.readFileSync("cli/tests/fixture.json");
  }
});

Deno.test("readFileDoesNotLeakResources", async function (): Promise<void> {
  const resourcesBefore = Deno.resources();
  await assertThrowsAsync(async () => await Deno.readFile("cli"));
  assertEquals(resourcesBefore, Deno.resources());
});

Deno.test("readFileSyncDoesNotLeakResources", function (): void {
  const resourcesBefore = Deno.resources();
  assertThrows(() => Deno.readFileSync("cli"));
  assertEquals(resourcesBefore, Deno.resources());
});

Deno.test("readFileSyncPerm", async function (): Promise<void> {
  await Deno.permissions.revoke({ name: "read" });

  assertThrows(() => {
    Deno.readFileSync("cli/tests/fixture.json");
  }, Deno.errors.PermissionDenied);
});

Deno.test("readFilePerm", async function (): Promise<
  void
> {
  await Deno.permissions.revoke({ name: "read" });

  await assertThrowsAsync(async () => {
    await Deno.readFile("cli/tests/fixture.json");
  }, Deno.errors.PermissionDenied);
});
