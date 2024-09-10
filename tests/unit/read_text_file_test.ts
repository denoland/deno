// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

import {
  assert,
  assertEquals,
  assertRejects,
  assertThrows,
  pathToAbsoluteFileUrl,
  unreachable,
} from "./test_util.ts";

Deno.test({ permissions: { read: true } }, function readTextFileSyncSuccess() {
  const data = Deno.readTextFileSync("tests/testdata/assets/fixture.json");
  assert(data.length > 0);
  const pkg = JSON.parse(data);
  assertEquals(pkg.name, "deno");
});

Deno.test({ permissions: { read: true } }, function readTextFileSyncByUrl() {
  const data = Deno.readTextFileSync(
    pathToAbsoluteFileUrl("tests/testdata/assets/fixture.json"),
  );
  assert(data.length > 0);
  const pkg = JSON.parse(data);
  assertEquals(pkg.name, "deno");
});

Deno.test({ permissions: { read: false } }, function readTextFileSyncPerm() {
  assertThrows(() => {
    Deno.readTextFileSync("tests/testdata/assets/fixture.json");
  }, Deno.errors.NotCapable);
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
      "tests/testdata/assets/fixture.json",
    );
    assert(data.length > 0);
    const pkg = JSON.parse(data);
    assertEquals(pkg.name, "deno");
  },
);

Deno.test({ permissions: { read: true } }, async function readTextFileByUrl() {
  const data = await Deno.readTextFile(
    pathToAbsoluteFileUrl("tests/testdata/assets/fixture.json"),
  );
  assert(data.length > 0);
  const pkg = JSON.parse(data);
  assertEquals(pkg.name, "deno");
});

Deno.test({ permissions: { read: false } }, async function readTextFilePerm() {
  await assertRejects(async () => {
    await Deno.readTextFile("tests/testdata/assets/fixture.json");
  }, Deno.errors.NotCapable);
});

Deno.test({ permissions: { read: true } }, function readTextFileSyncLoop() {
  for (let i = 0; i < 256; i++) {
    Deno.readTextFileSync("tests/testdata/assets/fixture.json");
  }
});

Deno.test(
  { permissions: { read: true } },
  async function readTextFileDoesNotLeakResources() {
    await assertRejects(async () => await Deno.readTextFile("cli"));
  },
);

Deno.test(
  { permissions: { read: true } },
  function readTextFileSyncDoesNotLeakResources() {
    assertThrows(() => Deno.readTextFileSync("cli"));
  },
);

Deno.test(
  { permissions: { read: true } },
  async function readTextFileWithAbortSignal() {
    const ac = new AbortController();
    queueMicrotask(() => ac.abort());
    const error = await assertRejects(
      async () => {
        await Deno.readTextFile("tests/testdata/assets/fixture.json", {
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
        await Deno.readTextFile("tests/testdata/assets/fixture.json", {
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
      await Deno.readTextFile("tests/testdata/assets/fixture.json", {
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
  async function readTextFileWithAbortSignalNotCalled() {
    const ac = new AbortController();
    await Deno.readTextFile("tests/testdata/assets/fixture.json", {
      signal: ac.signal,
    });
  },
);

Deno.test(
  { ignore: Deno.build.os !== "linux" },
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
    const filePath = "tests/testdata/too_big_a_file.txt";

    try {
      Deno.writeFileSync(filePath, bytes);
    } catch {
      // NOTE(bartlomieju): writing a 0.5Gb file might be too much for CI,
      // so skip running if writing fails.
      return;
    }

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
    const filePath = "tests/testdata/too_big_a_file_2.txt";

    try {
      await Deno.writeFile(filePath, bytes);
    } catch {
      // NOTE(bartlomieju): writing a 0.5Gb file might be too much for CI,
      // so skip running if writing fails.
      return;
    }

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
