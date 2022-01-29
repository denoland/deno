// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.
import { writeAllSync } from "../../../test_util/std/io/util.ts";
import {
  assert,
  assertEquals,
  assertRejects,
  assertThrows,
  pathToAbsoluteFileUrl,
  unreachable,
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
    await assertRejects(
      async () => {
        await Deno.readFile("cli/tests/testdata/fixture.json", {
          signal: ac.signal,
        });
      },
      (error: Error) => {
        assert(error instanceof DOMException);
        assertEquals(error.name, "AbortError");
      },
    );
  },
);

Deno.test(
  { permissions: { read: true } },
  async function readFileWithAbortSignalReason() {
    const ac = new AbortController();
    const abortReason = new Error();
    queueMicrotask(() => ac.abort(abortReason));
    await assertRejects(
      async () => {
        await Deno.readFile("cli/tests/testdata/fixture.json", {
          signal: ac.signal,
        });
      },
      (error: Error) => {
        assertEquals(error, abortReason);
      },
    );
  },
);

Deno.test(
  { permissions: { read: true } },
  async function readFileWithAbortSignalPrimitiveReason() {
    const ac = new AbortController();
    queueMicrotask(() => ac.abort("Some string"));
    try {
      await Deno.readFile("cli/tests/testdata/fixture.json", {
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
  async function readFileProcFs() {
    const data = await Deno.readFile("/proc/self/stat");
    assert(data.byteLength > 0);
  },
);

Deno.test(
  { permissions: { read: true, write: true } },
  async function readFileExtendedDuringRead() {
    // Write 128MB file
    const filename = Deno.makeTempDirSync() + "/test.txt";
    const data = new Uint8Array(1024 * 1024 * 128);
    Deno.writeFileSync(filename, data);
    const promise = Deno.readFile(filename);
    queueMicrotask(() => {
      // Append 128MB to file
      const f = Deno.openSync(filename, { append: true });
      writeAllSync(f, data);
      f.close();
    });
    const read = await promise;
    assertEquals(read.byteLength, data.byteLength * 2);
  },
);

Deno.test(
  { permissions: { read: true, write: true } },
  async function readFile0LengthExtendedDuringRead() {
    // Write 0 byte file
    const filename = Deno.makeTempDirSync() + "/test.txt";
    const first = new Uint8Array(0);
    const second = new Uint8Array(1024 * 1024 * 128);
    Deno.writeFileSync(filename, first);
    const promise = Deno.readFile(filename);
    queueMicrotask(() => {
      // Append 128MB to file
      const f = Deno.openSync(filename, { append: true });
      writeAllSync(f, second);
      f.close();
    });
    const read = await promise;
    assertEquals(read.byteLength, second.byteLength);
  },
);
