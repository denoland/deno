import {
  assert,
  assertEquals,
  assertThrows,
  assertThrowsAsync,
  pathToAbsoluteFileUrl,
  unitTest,
} from "./test_util.ts";

unitTest({ perms: { read: true } }, function readTextFileSyncSuccess() {
  const data = Deno.readTextFileSync("cli/tests/testdata/fixture.json");
  assert(data.length > 0);
  const pkg = JSON.parse(data);
  assertEquals(pkg.name, "deno");
});

unitTest({ perms: { read: true } }, function readTextFileSyncByUrl() {
  const data = Deno.readTextFileSync(
    pathToAbsoluteFileUrl("cli/tests/testdata/fixture.json"),
  );
  assert(data.length > 0);
  const pkg = JSON.parse(data);
  assertEquals(pkg.name, "deno");
});

unitTest({ perms: { read: false } }, function readTextFileSyncPerm() {
  assertThrows(() => {
    Deno.readTextFileSync("cli/tests/testdata/fixture.json");
  }, Deno.errors.PermissionDenied);
});

unitTest({ perms: { read: true } }, function readTextFileSyncNotFound() {
  assertThrows(() => {
    Deno.readTextFileSync("bad_filename");
  }, Deno.errors.NotFound);
});

unitTest(
  { perms: { read: true } },
  async function readTextFileSuccess() {
    const data = await Deno.readTextFile("cli/tests/testdata/fixture.json");
    assert(data.length > 0);
    const pkg = JSON.parse(data);
    assertEquals(pkg.name, "deno");
  },
);

unitTest({ perms: { read: true } }, async function readTextFileByUrl() {
  const data = await Deno.readTextFile(
    pathToAbsoluteFileUrl("cli/tests/testdata/fixture.json"),
  );
  assert(data.length > 0);
  const pkg = JSON.parse(data);
  assertEquals(pkg.name, "deno");
});

unitTest({ perms: { read: false } }, async function readTextFilePerm() {
  await assertThrowsAsync(async () => {
    await Deno.readTextFile("cli/tests/testdata/fixture.json");
  }, Deno.errors.PermissionDenied);
});

unitTest({ perms: { read: true } }, function readTextFileSyncLoop() {
  for (let i = 0; i < 256; i++) {
    Deno.readTextFileSync("cli/tests/testdata/fixture.json");
  }
});

unitTest(
  { perms: { read: true } },
  async function readTextFileDoesNotLeakResources() {
    const resourcesBefore = Deno.resources();
    await assertThrowsAsync(async () => await Deno.readTextFile("cli"));
    assertEquals(resourcesBefore, Deno.resources());
  },
);

unitTest(
  { perms: { read: true } },
  function readTextFileSyncDoesNotLeakResources() {
    const resourcesBefore = Deno.resources();
    assertThrows(() => Deno.readTextFileSync("cli"));
    assertEquals(resourcesBefore, Deno.resources());
  },
);
