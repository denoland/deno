// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
import {
  assert,
  assertEquals,
  assertRejects,
  assertThrows,
  pathToAbsoluteFileUrl,
  unreachable,
} from "./test_util.ts";

Deno.test({ permissions: { read: true } }, function readFileSyncSuccess() {
  const data = Deno.readFileSync("tests/testdata/assets/fixture.json");
  assert(data.byteLength > 0);
  const decoder = new TextDecoder("utf-8");
  const json = decoder.decode(data);
  const pkg = JSON.parse(json);
  assertEquals(pkg.name, "deno");
});

Deno.test({ permissions: { read: true } }, function readFileSyncUrl() {
  const data = Deno.readFileSync(
    pathToAbsoluteFileUrl("tests/testdata/assets/fixture.json"),
  );
  assert(data.byteLength > 0);
  const decoder = new TextDecoder("utf-8");
  const json = decoder.decode(data);
  const pkg = JSON.parse(json);
  assertEquals(pkg.name, "deno");
});

Deno.test({ permissions: { read: false } }, function readFileSyncPerm() {
  assertThrows(() => {
    Deno.readFileSync("tests/testdata/assets/fixture.json");
  }, Deno.errors.NotCapable);
});

Deno.test({ permissions: { read: true } }, function readFileSyncNotFound() {
  assertThrows(() => {
    Deno.readFileSync("bad_filename");
  }, Deno.errors.NotFound);
});

Deno.test({ permissions: { read: true } }, async function readFileUrl() {
  const data = await Deno.readFile(
    pathToAbsoluteFileUrl("tests/testdata/assets/fixture.json"),
  );
  assert(data.byteLength > 0);
  const decoder = new TextDecoder("utf-8");
  const json = decoder.decode(data);
  const pkg = JSON.parse(json);
  assertEquals(pkg.name, "deno");
});

Deno.test({ permissions: { read: true } }, async function readFileSuccess() {
  const data = await Deno.readFile("tests/testdata/assets/fixture.json");
  assert(data.byteLength > 0);
  const decoder = new TextDecoder("utf-8");
  const json = decoder.decode(data);
  const pkg = JSON.parse(json);
  assertEquals(pkg.name, "deno");
});

Deno.test({ permissions: { read: false } }, async function readFilePerm() {
  await assertRejects(async () => {
    await Deno.readFile("tests/testdata/assets/fixture.json");
  }, Deno.errors.NotCapable);
});

Deno.test({ permissions: { read: true } }, function readFileSyncLoop() {
  for (let i = 0; i < 256; i++) {
    Deno.readFileSync("tests/testdata/assets/fixture.json");
  }
});

Deno.test(
  { permissions: { read: true } },
  async function readFileDoesNotLeakResources() {
    await assertRejects(async () => await Deno.readFile("cli"));
  },
);

Deno.test(
  { permissions: { read: true } },
  function readFileSyncDoesNotLeakResources() {
    assertThrows(() => Deno.readFileSync("cli"));
  },
);

Deno.test(
  { permissions: { read: true } },
  async function readFileWithAbortSignal() {
    const ac = new AbortController();
    queueMicrotask(() => ac.abort());
    const error = await assertRejects(
      async () => {
        await Deno.readFile("tests/testdata/assets/fixture.json", {
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
  async function readFileWithAbortSignalReason() {
    const ac = new AbortController();
    const abortReason = new Error();
    queueMicrotask(() => ac.abort(abortReason));
    const error = await assertRejects(
      async () => {
        await Deno.readFile("tests/testdata/assets/fixture.json", {
          signal: ac.signal,
        });
      },
    );
    assertEquals(error, abortReason);
  },
);

Deno.test(
  { permissions: { read: true } },
  async function readFileWithAbortSignalPrimitiveReason() {
    const ac = new AbortController();
    queueMicrotask(() => ac.abort("Some string"));
    try {
      await Deno.readFile("tests/testdata/assets/fixture.json", {
        signal: ac.signal,
      });
      unreachable();
    } catch (e) {
      assertEquals(e, "Some string");
    }
  },
);

// Test that AbortController's cancel handle is cleaned-up correctly, and do not leak resources.
Deno.test(
  { permissions: { read: true } },
  async function readFileWithAbortSignalNotCalled() {
    const ac = new AbortController();
    await Deno.readFile("tests/testdata/assets/fixture.json", {
      signal: ac.signal,
    });
  },
);

Deno.test(
  { ignore: Deno.build.os !== "linux" },
  async function readFileProcFs() {
    const data = await Deno.readFile("/proc/self/stat");
    assert(data.byteLength > 0);
  },
);

Deno.test(
  { permissions: { read: true } },
  async function readFileNotFoundErrorCode() {
    try {
      await Deno.readFile("definitely-not-found.json");
    } catch (e) {
      assertEquals((e as { code: string }).code, "ENOENT");
    }
  },
);

Deno.test(
  { permissions: { read: true } },
  async function readFileIsDirectoryErrorCode() {
    try {
      await Deno.readFile("tests/testdata/assets/");
    } catch (e) {
      assertEquals((e as { code: string }).code, "EISDIR");
    }
  },
);
