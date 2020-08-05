import {
  unitTest,
  assert,
  assertEquals,
  assertThrows,
  assertThrowsAsync,
} from "./test_util.ts";

unitTest(
  { perms: { read: true, write: true } },
  function writeTextFileSyncSuccess(): void {
    const filename = Deno.makeTempDirSync() + "/test.txt";
    Deno.writeTextFileSync(filename, "Hello");
    const dataRead = Deno.readTextFileSync(filename);
    assertEquals("Hello", dataRead);
  },
);

unitTest(
  { perms: { read: true, write: true } },
  function writeTextFileSyncByUrl(): void {
    const tempDir = Deno.makeTempDirSync();
    const fileUrl = new URL(
      `file://${Deno.build.os === "windows" ? "/" : ""}${tempDir}/test.txt`,
    );
    Deno.writeTextFileSync(fileUrl, "Hello");
    const dataRead = Deno.readTextFileSync(fileUrl);
    assertEquals("Hello", dataRead);

    Deno.removeSync(fileUrl, { recursive: true });
  },
);

unitTest({ perms: { write: true } }, function writeTextFileSyncFail(): void {
  const filename = "/baddir/test.txt";
  // The following should fail because /baddir doesn't exist (hopefully).
  assertThrows(() => {
    Deno.writeTextFileSync(filename, "hello");
  }, Deno.errors.NotFound);
});

unitTest({ perms: { write: false } }, function writeTextFileSyncPerm(): void {
  const filename = "/baddir/test.txt";
  // The following should fail due to no write permission
  assertThrows(() => {
    Deno.writeTextFileSync(filename, "Hello");
  }, Deno.errors.PermissionDenied);
});

unitTest(
  { perms: { read: true, write: true } },
  function writeTextFileSyncUpdateMode(): void {
    if (Deno.build.os !== "windows") {
      const data = "Hello";
      const filename = Deno.makeTempDirSync() + "/test.txt";
      Deno.writeTextFileSync(filename, data, { mode: 0o755 });
      assertEquals(Deno.statSync(filename).mode! & 0o777, 0o755);
      Deno.writeTextFileSync(filename, data, { mode: 0o666 });
      assertEquals(Deno.statSync(filename).mode! & 0o777, 0o666);
    }
  },
);

unitTest(
  { perms: { read: true, write: true } },
  function writeTextFileSyncCreate(): void {
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
  },
);

unitTest(
  { perms: { read: true, write: true } },
  function writeTextFileSyncAppend(): void {
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
  },
);

unitTest(
  { perms: { read: true, write: true } },
  async function writeTextFileSuccess(): Promise<void> {
    const filename = Deno.makeTempDirSync() + "/test.txt";
    await Deno.writeTextFile(filename, "Hello");
    const dataRead = Deno.readTextFileSync(filename);
    assertEquals("Hello", dataRead);
  },
);

unitTest(
  { perms: { read: true, write: true } },
  async function writeTextFileByUrl(): Promise<void> {
    const tempDir = Deno.makeTempDirSync();
    const fileUrl = new URL(
      `file://${Deno.build.os === "windows" ? "/" : ""}${tempDir}/test.txt`,
    );
    await Deno.writeTextFile(fileUrl, "Hello");
    const dataRead = Deno.readTextFileSync(fileUrl);
    assertEquals("Hello", dataRead);

    Deno.removeSync(fileUrl, { recursive: true });
  },
);

unitTest(
  { perms: { read: true, write: true } },
  async function writeTextFileNotFound(): Promise<void> {
    const filename = "/baddir/test.txt";
    // The following should fail because /baddir doesn't exist (hopefully).
    await assertThrowsAsync(async () => {
      await Deno.writeTextFile(filename, "Hello");
    }, Deno.errors.NotFound);
  },
);

unitTest(
  { perms: { write: false } },
  async function writeTextFilePerm(): Promise<void> {
    const filename = "/baddir/test.txt";
    // The following should fail due to no write permission
    await assertThrowsAsync(async () => {
      await Deno.writeTextFile(filename, "Hello");
    }, Deno.errors.PermissionDenied);
  },
);

unitTest(
  { perms: { read: true, write: true } },
  async function writeTextFileUpdateMode(): Promise<void> {
    if (Deno.build.os !== "windows") {
      const data = "Hello";
      const filename = Deno.makeTempDirSync() + "/test.txt";
      await Deno.writeTextFile(filename, data, { mode: 0o755 });
      assertEquals(Deno.statSync(filename).mode! & 0o777, 0o755);
      await Deno.writeTextFile(filename, data, { mode: 0o666 });
      assertEquals(Deno.statSync(filename).mode! & 0o777, 0o666);
    }
  },
);

unitTest(
  { perms: { read: true, write: true } },
  async function writeTextFileCreate(): Promise<void> {
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
  },
);

unitTest(
  { perms: { read: true, write: true } },
  async function writeTextFileAppend(): Promise<void> {
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
  },
);
