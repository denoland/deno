// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import {
  assert,
  assertEquals,
  assertRejects,
  assertThrows,
  pathToAbsoluteFileUrl,
} from "./test_util.ts";

Deno.test({ permissions: { read: true } }, function readFileSyncSuccess() {
  const data = Deno.readFileSync("cli/tests/testdata/fixture.json");
  assert(data.byteLength > 0);
  const decoder = new TextDecoder("utf-8");
  const json = decoder.decode(data);
  const pkg = JSON.parse(json);
  assertEquals(pkg.name, "deno");
});

Deno.test({ permissions: { read: true } }, function readFileSyncUrl() {
  const data = Deno.readFileSync(
    pathToAbsoluteFileUrl("cli/tests/testdata/fixture.json"),
  );
  assert(data.byteLength > 0);
  const decoder = new TextDecoder("utf-8");
  const json = decoder.decode(data);
  const pkg = JSON.parse(json);
  assertEquals(pkg.name, "deno");
});

Deno.test({ permissions: { read: false } }, function readFileSyncPerm() {
  assertThrows(() => {
    Deno.readFileSync("cli/tests/testdata/fixture.json");
  }, Deno.errors.PermissionDenied);
});

Deno.test({ permissions: { read: true } }, function readFileSyncNotFound() {
  assertThrows(() => {
    Deno.readFileSync("bad_filename");
  }, Deno.errors.NotFound);
});

Deno.test({ permissions: { read: true } }, async function readFileUrl() {
  const data = await Deno.readFile(
    pathToAbsoluteFileUrl("cli/tests/testdata/fixture.json"),
  );
  assert(data.byteLength > 0);
  const decoder = new TextDecoder("utf-8");
  const json = decoder.decode(data);
  const pkg = JSON.parse(json);
  assertEquals(pkg.name, "deno");
});

Deno.test({ permissions: { read: true } }, async function readFileSuccess() {
  const data = await Deno.readFile("cli/tests/testdata/fixture.json");
  assert(data.byteLength > 0);
  const decoder = new TextDecoder("utf-8");
  const json = decoder.decode(data);
  const pkg = JSON.parse(json);
  assertEquals(pkg.name, "deno");
});

Deno.test({ permissions: { read: false } }, async function readFilePerm() {
  await assertRejects(async () => {
    await Deno.readFile("cli/tests/testdata/fixture.json");
  }, Deno.errors.PermissionDenied);
});

Deno.test({ permissions: { read: true } }, function readFileSyncLoop() {
  for (let i = 0; i < 256; i++) {
    Deno.readFileSync("cli/tests/testdata/fixture.json");
  }
});

Deno.test(
  { permissions: { read: true } },
  async function readFileDoesNotLeakResources() {
    const resourcesBefore = Deno.resources();
    await assertRejects(async () => await Deno.readFile("cli"));
    assertEquals(resourcesBefore, Deno.resources());
  },
);

Deno.test(
  { permissions: { read: true } },
  function readFileSyncDoesNotLeakResources() {
    const resourcesBefore = Deno.resources();
    assertThrows(() => Deno.readFileSync("cli"));
    assertEquals(resourcesBefore, Deno.resources());
  },
);

Deno.test(
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

Deno.test(
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
