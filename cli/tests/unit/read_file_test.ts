// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import {
  assert,
  assertEquals,
  assertRejects,
  assertThrows,
  pathToAbsoluteFileUrl,
  unitTest,
} from "./test_util.ts";

unitTest({ permissions: { read: true } }, function readFileSyncSuccess() {
  const data = Deno.readFileSync("cli/tests/testdata/fixture.json");
  assert(data.byteLength > 0);
  const decoder = new TextDecoder("utf-8");
  const json = decoder.decode(data);
  const pkg = JSON.parse(json);
  assertEquals(pkg.name, "deno");
});

unitTest({ permissions: { read: true } }, function readFileSyncUrl() {
  const data = Deno.readFileSync(
    pathToAbsoluteFileUrl("cli/tests/testdata/fixture.json"),
  );
  assert(data.byteLength > 0);
  const decoder = new TextDecoder("utf-8");
  const json = decoder.decode(data);
  const pkg = JSON.parse(json);
  assertEquals(pkg.name, "deno");
});

unitTest({ permissions: { read: false } }, function readFileSyncPerm() {
  assertThrows(() => {
    Deno.readFileSync("cli/tests/testdata/fixture.json");
  }, Deno.errors.PermissionDenied);
});

unitTest({ permissions: { read: true } }, function readFileSyncNotFound() {
  assertThrows(() => {
    Deno.readFileSync("bad_filename");
  }, Deno.errors.NotFound);
});

unitTest({ permissions: { read: true } }, async function readFileUrl() {
  const data = await Deno.readFile(
    pathToAbsoluteFileUrl("cli/tests/testdata/fixture.json"),
  );
  assert(data.byteLength > 0);
  const decoder = new TextDecoder("utf-8");
  const json = decoder.decode(data);
  const pkg = JSON.parse(json);
  assertEquals(pkg.name, "deno");
});

unitTest({ permissions: { read: true } }, async function readFileSuccess() {
  const data = await Deno.readFile("cli/tests/testdata/fixture.json");
  assert(data.byteLength > 0);
  const decoder = new TextDecoder("utf-8");
  const json = decoder.decode(data);
  const pkg = JSON.parse(json);
  assertEquals(pkg.name, "deno");
});

unitTest({ permissions: { read: false } }, async function readFilePerm() {
  await assertRejects(async () => {
    await Deno.readFile("cli/tests/testdata/fixture.json");
  }, Deno.errors.PermissionDenied);
});

unitTest({ permissions: { read: true } }, function readFileSyncLoop() {
  for (let i = 0; i < 256; i++) {
    Deno.readFileSync("cli/tests/testdata/fixture.json");
  }
});

unitTest(
  { permissions: { read: true } },
  async function readFileDoesNotLeakResources() {
    const resourcesBefore = Deno.resources();
    await assertRejects(async () => await Deno.readFile("cli"));
    assertEquals(resourcesBefore, Deno.resources());
  },
);

unitTest(
  { permissions: { read: true } },
  function readFileSyncDoesNotLeakResources() {
    const resourcesBefore = Deno.resources();
    assertThrows(() => Deno.readFileSync("cli"));
    assertEquals(resourcesBefore, Deno.resources());
  },
);

unitTest(
  { permissions: { read: true } },
  async function readFileWithAbortSignal() {
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
  { permissions: { read: true } },
  async function readTextileWithAbortSignal() {
    const ac = new AbortController();
    queueMicrotask(() => ac.abort());
    await assertRejects(async () => {
      await Deno.readTextFile("cli/tests/testdata/fixture.json", {
        signal: ac.signal,
      });
    });
  },
);
