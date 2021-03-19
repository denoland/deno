import {
  assert,
  assertEquals,
  assertThrows,
  assertThrowsAsync,
} from "./test_util.ts";

Deno.test("writeTextFileSyncSuccess", function (): void {
  const filename = Deno.makeTempDirSync() + "/test.txt";
  Deno.writeTextFileSync(filename, "Hello");
  const dataRead = Deno.readTextFileSync(filename);
  assertEquals("Hello", dataRead);
});

Deno.test("writeTextFileSyncByUrl", function (): void {
  const tempDir = Deno.makeTempDirSync();
  const fileUrl = new URL(
    `file://${Deno.build.os === "windows" ? "/" : ""}${tempDir}/test.txt`,
  );
  Deno.writeTextFileSync(fileUrl, "Hello");
  const dataRead = Deno.readTextFileSync(fileUrl);
  assertEquals("Hello", dataRead);

  Deno.removeSync(fileUrl, { recursive: true });
});

Deno.test("writeTextFileSyncFail", function (): void {
  const filename = "/baddir/test.txt";
  // The following should fail because /baddir doesn't exist (hopefully).
  assertThrows(() => {
    Deno.writeTextFileSync(filename, "hello");
  }, Deno.errors.NotFound);
});

Deno.test("writeTextFileSyncUpdateMode", function (): void {
  if (Deno.build.os !== "windows") {
    const data = "Hello";
    const filename = Deno.makeTempDirSync() + "/test.txt";
    Deno.writeTextFileSync(filename, data, { mode: 0o755 });
    assertEquals(Deno.statSync(filename).mode! & 0o777, 0o755);
    Deno.writeTextFileSync(filename, data, { mode: 0o666 });
    assertEquals(Deno.statSync(filename).mode! & 0o777, 0o666);
  }
});

Deno.test("writeTextFileSyncCreate", function (): void {
  const data = "Hello";
  const filename = Deno.makeTempDirSync() + "/test.txt";
  let caughtError = false;
  // if create turned off, the file won't be created
  try {
    Deno.writeTextFileSync(filename, data, { create: false });
  } catch (e) {
    caughtError = true;
    assert(e instanceof Deno.errors.NotFound);
  }
  assert(caughtError);

  // Turn on create, should have no error
  Deno.writeTextFileSync(filename, data, { create: true });
  Deno.writeTextFileSync(filename, data, { create: false });
  assertEquals("Hello", Deno.readTextFileSync(filename));
});

Deno.test("writeTextFileSyncAppend", function (): void {
  const data = "Hello";
  const filename = Deno.makeTempDirSync() + "/test.txt";
  Deno.writeTextFileSync(filename, data);
  Deno.writeTextFileSync(filename, data, { append: true });
  assertEquals("HelloHello", Deno.readTextFileSync(filename));
  // Now attempt overwrite
  Deno.writeTextFileSync(filename, data, { append: false });
  assertEquals("Hello", Deno.readTextFileSync(filename));
  // append not set should also overwrite
  Deno.writeTextFileSync(filename, data);
  assertEquals("Hello", Deno.readTextFileSync(filename));
});

Deno.test("writeTextFileSuccess", async function (): Promise<void> {
  const filename = Deno.makeTempDirSync() + "/test.txt";
  await Deno.writeTextFile(filename, "Hello");
  const dataRead = Deno.readTextFileSync(filename);
  assertEquals("Hello", dataRead);
});

Deno.test("writeTextFileByUrl", async function (): Promise<void> {
  const tempDir = Deno.makeTempDirSync();
  const fileUrl = new URL(
    `file://${Deno.build.os === "windows" ? "/" : ""}${tempDir}/test.txt`,
  );
  await Deno.writeTextFile(fileUrl, "Hello");
  const dataRead = Deno.readTextFileSync(fileUrl);
  assertEquals("Hello", dataRead);

  Deno.removeSync(fileUrl, { recursive: true });
});

Deno.test("writeTextFileNotFound", async function (): Promise<void> {
  const filename = "/baddir/test.txt";
  // The following should fail because /baddir doesn't exist (hopefully).
  await assertThrowsAsync(async () => {
    await Deno.writeTextFile(filename, "Hello");
  }, Deno.errors.NotFound);
});

Deno.test("writeTextFileUpdateMode", async function (): Promise<void> {
  if (Deno.build.os !== "windows") {
    const data = "Hello";
    const filename = Deno.makeTempDirSync() + "/test.txt";
    await Deno.writeTextFile(filename, data, { mode: 0o755 });
    assertEquals(Deno.statSync(filename).mode! & 0o777, 0o755);
    await Deno.writeTextFile(filename, data, { mode: 0o666 });
    assertEquals(Deno.statSync(filename).mode! & 0o777, 0o666);
  }
});

Deno.test("writeTextFileCreate", async function (): Promise<void> {
  const data = "Hello";
  const filename = Deno.makeTempDirSync() + "/test.txt";
  let caughtError = false;
  // if create turned off, the file won't be created
  try {
    await Deno.writeTextFile(filename, data, { create: false });
  } catch (e) {
    caughtError = true;
    assert(e instanceof Deno.errors.NotFound);
  }
  assert(caughtError);

  // Turn on create, should have no error
  await Deno.writeTextFile(filename, data, { create: true });
  await Deno.writeTextFile(filename, data, { create: false });
  assertEquals("Hello", Deno.readTextFileSync(filename));
});

Deno.test("writeTextFileAppend", async function (): Promise<void> {
  const data = "Hello";
  const filename = Deno.makeTempDirSync() + "/test.txt";
  await Deno.writeTextFile(filename, data);
  await Deno.writeTextFile(filename, data, { append: true });
  assertEquals("HelloHello", Deno.readTextFileSync(filename));
  // Now attempt overwrite
  await Deno.writeTextFile(filename, data, { append: false });
  assertEquals("Hello", Deno.readTextFileSync(filename));
  // append not set should also overwrite
  await Deno.writeTextFile(filename, data);
  assertEquals("Hello", Deno.readTextFileSync(filename));
});

Deno.test("writeTextFileSyncPerm", async function (): Promise<void> {
  await Deno.permissions.revoke({ name: "write" });

  const filename = "/baddir/test.txt";
  // The following should fail due to no write permission
  assertThrows(() => {
    Deno.writeTextFileSync(filename, "Hello");
  }, Deno.errors.PermissionDenied);
});

Deno.test("writeTextFilePerm", async function (): Promise<void> {
  await Deno.permissions.revoke({ name: "write" });

  const filename = "/baddir/test.txt";
  // The following should fail due to no write permission
  await assertThrowsAsync(async () => {
    await Deno.writeTextFile(filename, "Hello");
  }, Deno.errors.PermissionDenied);
});

Deno.test("writeTextFilePerm", async function (): Promise<void> {
  const filename = "/baddir/test.txt";
  // The following should fail due to no write permission
  await assertThrowsAsync(async () => {
    await Deno.writeTextFile(filename, "Hello");
  }, Deno.errors.PermissionDenied);
});
