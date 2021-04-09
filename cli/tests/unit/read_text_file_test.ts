import {
  assert,
  assertEquals,
  assertThrows,
  assertThrowsAsync,
  pathToAbsoluteFileUrl,
} from "./test_util.ts";

Deno.test("readTextFileSyncSuccess", function (): void {
  const data = Deno.readTextFileSync("cli/tests/fixture.json");
  assert(data.length > 0);
  const pkg = JSON.parse(data);
  assertEquals(pkg.name, "deno");
});

Deno.test("readTextFileSyncByUrl", function (): void {
  const data = Deno.readTextFileSync(
    pathToAbsoluteFileUrl("cli/tests/fixture.json"),
  );
  assert(data.length > 0);
  const pkg = JSON.parse(data);
  assertEquals(pkg.name, "deno");
});

Deno.test("readTextFileSyncNotFound", function (): void {
  assertThrows(() => {
    Deno.readTextFileSync("bad_filename");
  }, Deno.errors.NotFound);
});

Deno.test("readTextFileSuccess", async function (): Promise<void> {
  const data = await Deno.readTextFile("cli/tests/fixture.json");
  assert(data.length > 0);
  const pkg = JSON.parse(data);
  assertEquals(pkg.name, "deno");
});

Deno.test("readTextFileByUrl", async function (): Promise<
  void
> {
  const data = await Deno.readTextFile(
    pathToAbsoluteFileUrl("cli/tests/fixture.json"),
  );
  assert(data.length > 0);
  const pkg = JSON.parse(data);
  assertEquals(pkg.name, "deno");
});

Deno.test("readTextFileSyncLoop", function (): void {
  for (let i = 0; i < 256; i++) {
    Deno.readTextFileSync("cli/tests/fixture.json");
  }
});

Deno.test("readTextFilePerm", async function (): Promise<
  void
> {
  await Deno.permissions.revoke({ name: "read" });

  await assertThrowsAsync(async () => {
    await Deno.readTextFile("cli/tests/fixture.json");
  }, Deno.errors.PermissionDenied);
});

Deno.test("readTextFileSyncPerm", async function (): Promise<void> {
  await Deno.permissions.revoke({ name: "read" });

  assertThrows(() => {
    Deno.readTextFileSync("cli/tests/fixture.json");
  }, Deno.errors.PermissionDenied);
});

Deno.test("readTextFileDoesNotLeakResources()", async function (): Promise<
  void
> {
  const resourcesBefore = Deno.resources();
  await assertThrowsAsync(async () => await Deno.readTextFile("cli"));
  assertEquals(resourcesBefore, Deno.resources());
});

Deno.test("readTextFileDoesNotLeakResources", function (): void {
  const resourcesBefore = Deno.resources();
  assertThrows(() => Deno.readTextFileSync("cli"));
  assertEquals(resourcesBefore, Deno.resources());
});
