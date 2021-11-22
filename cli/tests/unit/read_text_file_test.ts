import {
  assert,
  assertEquals,
  assertRejects,
  assertThrows,
  pathToAbsoluteFileUrl,
  unitTest,
} from "./test_util.ts";

unitTest({ permissions: { read: true } }, function readTextFileSyncSuccess() {
  const data = Deno.readTextFileSync("cli/tests/testdata/fixture.json");
  assert(data.length > 0);
  const pkg = JSON.parse(data);
  assertEquals(pkg.name, "deno");
});

unitTest({ permissions: { read: true } }, function readTextFileSyncByUrl() {
  const data = Deno.readTextFileSync(
    pathToAbsoluteFileUrl("cli/tests/testdata/fixture.json"),
  );
  assert(data.length > 0);
  const pkg = JSON.parse(data);
  assertEquals(pkg.name, "deno");
});

unitTest({ permissions: { read: false } }, function readTextFileSyncPerm() {
  assertThrows(() => {
    Deno.readTextFileSync("cli/tests/testdata/fixture.json");
  }, Deno.errors.PermissionDenied);
});

unitTest({ permissions: { read: true } }, function readTextFileSyncNotFound() {
  assertThrows(() => {
    Deno.readTextFileSync("bad_filename");
  }, Deno.errors.NotFound);
});

unitTest(
  { permissions: { read: true } },
  async function readTextFileSuccess() {
    const data = await Deno.readTextFile("cli/tests/testdata/fixture.json");
    assert(data.length > 0);
    const pkg = JSON.parse(data);
    assertEquals(pkg.name, "deno");
  },
);

unitTest({ permissions: { read: true } }, async function readTextFileByUrl() {
  const data = await Deno.readTextFile(
    pathToAbsoluteFileUrl("cli/tests/testdata/fixture.json"),
  );
  assert(data.length > 0);
  const pkg = JSON.parse(data);
  assertEquals(pkg.name, "deno");
});

unitTest({ permissions: { read: false } }, async function readTextFilePerm() {
  await assertRejects(async () => {
    await Deno.readTextFile("cli/tests/testdata/fixture.json");
  }, Deno.errors.PermissionDenied);
});

unitTest({ permissions: { read: true } }, function readTextFileSyncLoop() {
  for (let i = 0; i < 256; i++) {
    Deno.readTextFileSync("cli/tests/testdata/fixture.json");
  }
});

unitTest(
  { permissions: { read: true } },
  async function readTextFileDoesNotLeakResources() {
    const resourcesBefore = Deno.resources();
    await assertRejects(async () => await Deno.readTextFile("cli"));
    assertEquals(resourcesBefore, Deno.resources());
  },
);

unitTest(
  { permissions: { read: true } },
  function readTextFileSyncDoesNotLeakResources() {
    const resourcesBefore = Deno.resources();
    assertThrows(() => Deno.readTextFileSync("cli"));
    assertEquals(resourcesBefore, Deno.resources());
  },
);

unitTest(
  { permissions: { read: true } },
  async function readTextFileWithAbortSignal() {
    const ac = new AbortController();
    queueMicrotask(() => ac.abort());
    await assertRejects(async () => {
      await Deno.readFile("cli/tests/testdata/fixture.json", {
        signal: ac.signal,
      });
    });
  },
);

unitTest(
  { permissions: { read: true }, ignore: Deno.build.os !== "linux" },
  async function readTextFileProcFs() {
    const data = await Deno.readTextFile("/proc/self/stat");
    assert(data.length > 0);
  },
);
