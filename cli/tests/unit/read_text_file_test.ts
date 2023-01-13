import {
  assert,
  assertEquals,
  assertRejects,
  assertThrows,
  pathToAbsoluteFileUrl,
  unreachable,
} from "./test_util.ts";

Deno.test({ permissions: { read: true } }, function readTextFileSyncSuccess() {
  const data = Deno.readTextFileSync("cli/tests/testdata/assets/fixture.json");
  assert(data.length > 0);
  const pkg = JSON.parse(data);
  assertEquals(pkg.name, "deno");
});

Deno.test({ permissions: { read: true } }, function readTextFileSyncByUrl() {
  const data = Deno.readTextFileSync(
    pathToAbsoluteFileUrl("cli/tests/testdata/assets/fixture.json"),
  );
  assert(data.length > 0);
  const pkg = JSON.parse(data);
  assertEquals(pkg.name, "deno");
});

Deno.test({ permissions: { read: false } }, function readTextFileSyncPerm() {
  assertThrows(() => {
    Deno.readTextFileSync("cli/tests/testdata/assets/fixture.json");
  }, Deno.errors.PermissionDenied);
});

Deno.test({ permissions: { read: true } }, function readTextFileSyncNotFound() {
  assertThrows(() => {
    Deno.readTextFileSync("bad_filename");
  }, Deno.errors.NotFound);
});

Deno.test(
  { permissions: { read: true } },
  async function readTextFileSuccess() {
    const data = await Deno.readTextFile(
      "cli/tests/testdata/assets/fixture.json",
    );
    assert(data.length > 0);
    const pkg = JSON.parse(data);
    assertEquals(pkg.name, "deno");
  },
);

Deno.test({ permissions: { read: true } }, async function readTextFileByUrl() {
  const data = await Deno.readTextFile(
    pathToAbsoluteFileUrl("cli/tests/testdata/assets/fixture.json"),
  );
  assert(data.length > 0);
  const pkg = JSON.parse(data);
  assertEquals(pkg.name, "deno");
});

Deno.test({ permissions: { read: false } }, async function readTextFilePerm() {
  await assertRejects(async () => {
    await Deno.readTextFile("cli/tests/testdata/assets/fixture.json");
  }, Deno.errors.PermissionDenied);
});

Deno.test({ permissions: { read: true } }, function readTextFileSyncLoop() {
  for (let i = 0; i < 256; i++) {
    Deno.readTextFileSync("cli/tests/testdata/assets/fixture.json");
  }
});

Deno.test(
  { permissions: { read: true } },
  async function readTextFileDoesNotLeakResources() {
    const resourcesBefore = Deno.resources();
    await assertRejects(async () => await Deno.readTextFile("cli"));
    assertEquals(resourcesBefore, Deno.resources());
  },
);

Deno.test(
  { permissions: { read: true } },
  function readTextFileSyncDoesNotLeakResources() {
    const resourcesBefore = Deno.resources();
    assertThrows(() => Deno.readTextFileSync("cli"));
    assertEquals(resourcesBefore, Deno.resources());
  },
);

Deno.test(
  { permissions: { read: true } },
  async function readTextFileWithAbortSignal() {
    const ac = new AbortController();
    queueMicrotask(() => ac.abort());
    const error = await assertRejects(
      async () => {
        await Deno.readFile("cli/tests/testdata/assets/fixture.json", {
          signal: ac.signal,
        });
      },
    );
    assert(error instanceof DOMException);
    assertEquals(error.name, "AbortError");
  },
);

Deno.test(
  { permissions: { read: true } },
  async function readTextFileWithAbortSignalReason() {
    const ac = new AbortController();
    const abortReason = new Error();
    queueMicrotask(() => ac.abort(abortReason));
    const error = await assertRejects(
      async () => {
        await Deno.readFile("cli/tests/testdata/assets/fixture.json", {
          signal: ac.signal,
        });
      },
    );
    assertEquals(error, abortReason);
  },
);

Deno.test(
  { permissions: { read: true } },
  async function readTextFileWithAbortSignalPrimitiveReason() {
    const ac = new AbortController();
    queueMicrotask(() => ac.abort("Some string"));
    try {
      await Deno.readFile("cli/tests/testdata/assets/fixture.json", {
        signal: ac.signal,
      });
      unreachable();
    } catch (e) {
      assertEquals(e, "Some string");
    }
  },
);

Deno.test(
  { permissions: { read: true }, ignore: Deno.build.os !== "linux" },
  async function readTextFileProcFs() {
    const data = await Deno.readTextFile("/proc/self/stat");
    assert(data.length > 0);
  },
);

Deno.test(
  { permissions: { read: true, write: true } },
  function readTextFileSyncV8LimitError() {
    const kStringMaxLengthPlusOne = 536870888 + 1;
    const bytes = new Uint8Array(kStringMaxLengthPlusOne);
    const filePath = "cli/tests/testdata/too_big_a_file.txt";

    Deno.writeFileSync(filePath, bytes);

    assertThrows(
      () => {
        Deno.readTextFileSync(filePath);
      },
      TypeError,
      "buffer exceeds maximum length",
    );

    Deno.removeSync(filePath);
  },
);

Deno.test(
  { permissions: { read: true, write: true } },
  async function readTextFileV8LimitError() {
    const kStringMaxLengthPlusOne = 536870888 + 1;
    const bytes = new Uint8Array(kStringMaxLengthPlusOne);
    const filePath = "cli/tests/testdata/too_big_a_file_2.txt";

    await Deno.writeFile(filePath, bytes);

    await assertRejects(
      async () => {
        await Deno.readTextFile(filePath);
      },
      TypeError,
      "buffer exceeds maximum length",
    );

    await Deno.remove(filePath);
  },
);
