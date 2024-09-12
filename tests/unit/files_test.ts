// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

import {
  assert,
  assertEquals,
  assertRejects,
  assertThrows,
} from "./test_util.ts";
import { copy } from "@std/io/copy";

// Note tests for Deno.FsFile.setRaw is in integration tests.

Deno.test(function filesStdioFileDescriptors() {
  // @ts-ignore `Deno.stdin.rid` was soft-removed in Deno 2.
  assertEquals(Deno.stdin.rid, 0);
  // @ts-ignore `Deno.stdout.rid` was soft-removed in Deno 2.
  assertEquals(Deno.stdout.rid, 1);
  // @ts-ignore `Deno.stderr.rid` was soft-removed in Deno 2.
  assertEquals(Deno.stderr.rid, 2);
});

Deno.test(
  { permissions: { read: true } },
  async function filesCopyToStdout() {
    const filename = "tests/testdata/assets/fixture.json";
    using file = await Deno.open(filename);
    assert(file instanceof Deno.FsFile);
    const bytesWritten = await copy(file, Deno.stdout);
    const fileSize = Deno.statSync(filename).size;
    assertEquals(bytesWritten, fileSize);
  },
);

Deno.test(
  {
    permissions: { read: true, write: true },
  },
  function openSyncMode() {
    const path = Deno.makeTempDirSync() + "/test_openSync.txt";
    using _file = Deno.openSync(path, {
      write: true,
      createNew: true,
      mode: 0o626,
    });
    const pathInfo = Deno.statSync(path);
    if (Deno.build.os !== "windows") {
      assertEquals(pathInfo.mode! & 0o777, 0o626 & ~Deno.umask());
    }
  },
);

Deno.test(
  {
    permissions: { read: true, write: true },
  },
  async function openMode() {
    const path = (await Deno.makeTempDir()) + "/test_open.txt";
    using _file = await Deno.open(path, {
      write: true,
      createNew: true,
      mode: 0o626,
    });
    const pathInfo = Deno.statSync(path);
    if (Deno.build.os !== "windows") {
      assertEquals(pathInfo.mode! & 0o777, 0o626 & ~Deno.umask());
    }
  },
);

Deno.test(
  {
    permissions: { read: true, write: true },
  },
  function openSyncUrl() {
    const tempDir = Deno.makeTempDirSync();
    const fileUrl = new URL(
      `file://${
        Deno.build.os === "windows" ? "/" : ""
      }${tempDir}/test_open.txt`,
    );
    using _file = Deno.openSync(fileUrl, {
      write: true,
      createNew: true,
      mode: 0o626,
    });
    const pathInfo = Deno.statSync(fileUrl);
    if (Deno.build.os !== "windows") {
      assertEquals(pathInfo.mode! & 0o777, 0o626 & ~Deno.umask());
    }

    Deno.removeSync(tempDir, { recursive: true });
  },
);

Deno.test(
  {
    permissions: { read: true, write: true },
  },
  async function openUrl() {
    const tempDir = await Deno.makeTempDir();
    const fileUrl = new URL(
      `file://${
        Deno.build.os === "windows" ? "/" : ""
      }${tempDir}/test_open.txt`,
    );
    using _file = await Deno.open(fileUrl, {
      write: true,
      createNew: true,
      mode: 0o626,
    });
    const pathInfo = Deno.statSync(fileUrl);
    if (Deno.build.os !== "windows") {
      assertEquals(pathInfo.mode! & 0o777, 0o626 & ~Deno.umask());
    }

    Deno.removeSync(tempDir, { recursive: true });
  },
);

Deno.test(
  { permissions: { write: false } },
  async function writePermFailure() {
    const filename = "tests/hello.txt";
    const openOptions: Deno.OpenOptions[] = [{ write: true }, { append: true }];
    for (const options of openOptions) {
      await assertRejects(async () => {
        await Deno.open(filename, options);
      }, Deno.errors.NotCapable);
    }
  },
);

Deno.test(async function openOptions() {
  const filename = "tests/testdata/assets/fixture.json";
  await assertRejects(
    async () => {
      await Deno.open(filename, { write: false });
    },
    Error,
    "OpenOptions requires at least one option to be true",
  );

  await assertRejects(
    async () => {
      await Deno.open(filename, { truncate: true, write: false });
    },
    Error,
    "'truncate' option requires 'write' option",
  );

  await assertRejects(
    async () => {
      await Deno.open(filename, { create: true, write: false });
    },
    Error,
    "'create' or 'createNew' options require 'write' or 'append' option",
  );

  await assertRejects(
    async () => {
      await Deno.open(filename, { createNew: true, append: false });
    },
    Error,
    "'create' or 'createNew' options require 'write' or 'append' option",
  );
});

Deno.test({ permissions: { read: false } }, async function readPermFailure() {
  await assertRejects(async () => {
    await Deno.open("package.json", { read: true });
  }, Deno.errors.NotCapable);
});

Deno.test(
  { permissions: { write: true } },
  async function writeNullBufferFailure() {
    const tempDir = Deno.makeTempDirSync();
    const filename = tempDir + "hello.txt";
    const w = {
      write: true,
      truncate: true,
      create: true,
    };
    using file = await Deno.open(filename, w);

    // writing null should throw an error
    await assertRejects(
      async () => {
        // deno-lint-ignore no-explicit-any
        await file.write(null as any);
      },
    ); // TODO(bartlomieju): Check error kind when dispatch_minimal pipes errors properly
    await Deno.remove(tempDir, { recursive: true });
  },
);

Deno.test(
  { permissions: { write: true, read: true } },
  async function readNullBufferFailure() {
    const tempDir = Deno.makeTempDirSync();
    const filename = tempDir + "hello.txt";
    using file = await Deno.open(filename, {
      read: true,
      write: true,
      truncate: true,
      create: true,
    });

    // reading into an empty buffer should return 0 immediately
    const bytesRead = await file.read(new Uint8Array(0));
    assert(bytesRead === 0);

    // reading file into null buffer should throw an error
    await assertRejects(async () => {
      // deno-lint-ignore no-explicit-any
      await file.read(null as any);
    }, TypeError);
    // TODO(bartlomieju): Check error kind when dispatch_minimal pipes errors properly

    await Deno.remove(tempDir, { recursive: true });
  },
);

Deno.test(
  { permissions: { write: false, read: false } },
  async function readWritePermFailure() {
    const filename = "tests/hello.txt";
    await assertRejects(async () => {
      await Deno.open(filename, { read: true });
    }, Deno.errors.NotCapable);
  },
);

Deno.test(
  { permissions: { write: true, read: true } },
  async function openNotFound() {
    await assertRejects(
      async () => {
        await Deno.open("bad_file_name");
      },
      Deno.errors.NotFound,
      `open 'bad_file_name'`,
    );
  },
);

Deno.test(
  { permissions: { write: true, read: true } },
  function openSyncNotFound() {
    assertThrows(
      () => {
        Deno.openSync("bad_file_name");
      },
      Deno.errors.NotFound,
      `open 'bad_file_name'`,
    );
  },
);

Deno.test(
  { permissions: { read: true, write: true } },
  async function createFile() {
    const tempDir = await Deno.makeTempDir();
    const filename = tempDir + "/test.txt";
    const f = await Deno.create(filename);
    let fileInfo = Deno.statSync(filename);
    assert(fileInfo.isFile);
    assert(fileInfo.size === 0);
    const enc = new TextEncoder();
    const data = enc.encode("Hello");
    await f.write(data);
    fileInfo = Deno.statSync(filename);
    assert(fileInfo.size === 5);
    f.close();

    // TODO(bartlomieju): test different modes
    await Deno.remove(tempDir, { recursive: true });
  },
);

Deno.test(
  { permissions: { read: true, write: true } },
  async function createFileWithUrl() {
    const tempDir = await Deno.makeTempDir();
    const fileUrl = new URL(
      `file://${Deno.build.os === "windows" ? "/" : ""}${tempDir}/test.txt`,
    );
    const f = await Deno.create(fileUrl);
    let fileInfo = Deno.statSync(fileUrl);
    assert(fileInfo.isFile);
    assert(fileInfo.size === 0);
    const enc = new TextEncoder();
    const data = enc.encode("Hello");
    await f.write(data);
    fileInfo = Deno.statSync(fileUrl);
    assert(fileInfo.size === 5);
    f.close();

    await Deno.remove(tempDir, { recursive: true });
  },
);

Deno.test(
  { permissions: { read: true, write: true } },
  async function createSyncFile() {
    const tempDir = await Deno.makeTempDir();
    const filename = tempDir + "/test.txt";
    const f = Deno.createSync(filename);
    let fileInfo = Deno.statSync(filename);
    assert(fileInfo.isFile);
    assert(fileInfo.size === 0);
    const enc = new TextEncoder();
    const data = enc.encode("Hello");
    await f.write(data);
    fileInfo = Deno.statSync(filename);
    assert(fileInfo.size === 5);
    f.close();

    // TODO(bartlomieju): test different modes
    await Deno.remove(tempDir, { recursive: true });
  },
);

Deno.test(
  { permissions: { read: true, write: true } },
  async function createSyncFileWithUrl() {
    const tempDir = await Deno.makeTempDir();
    const fileUrl = new URL(
      `file://${Deno.build.os === "windows" ? "/" : ""}${tempDir}/test.txt`,
    );
    const f = Deno.createSync(fileUrl);
    let fileInfo = Deno.statSync(fileUrl);
    assert(fileInfo.isFile);
    assert(fileInfo.size === 0);
    const enc = new TextEncoder();
    const data = enc.encode("Hello");
    await f.write(data);
    fileInfo = Deno.statSync(fileUrl);
    assert(fileInfo.size === 5);
    f.close();

    await Deno.remove(tempDir, { recursive: true });
  },
);

Deno.test(
  { permissions: { read: true, write: true } },
  async function openModeWrite() {
    const tempDir = Deno.makeTempDirSync();
    const encoder = new TextEncoder();
    const filename = tempDir + "hello.txt";
    const data = encoder.encode("Hello world!\n");
    let file = await Deno.open(filename, {
      create: true,
      write: true,
      truncate: true,
    });
    // assert file was created
    let fileInfo = Deno.statSync(filename);
    assert(fileInfo.isFile);
    assertEquals(fileInfo.size, 0);
    // write some data
    await file.write(data);
    fileInfo = Deno.statSync(filename);
    assertEquals(fileInfo.size, 13);
    // assert we can't read from file
    let thrown = false;
    try {
      const buf = new Uint8Array(20);
      await file.read(buf);
    } catch (_e) {
      thrown = true;
    } finally {
      assert(thrown, "'w' mode shouldn't allow to read file");
    }
    file.close();
    // assert that existing file is truncated on open
    file = await Deno.open(filename, {
      write: true,
      truncate: true,
    });
    file.close();
    const fileSize = Deno.statSync(filename).size;
    assertEquals(fileSize, 0);
    await Deno.remove(tempDir, { recursive: true });
  },
);

Deno.test(
  { permissions: { read: true, write: true } },
  async function openModeWriteRead() {
    const tempDir = Deno.makeTempDirSync();
    const encoder = new TextEncoder();
    const filename = tempDir + "hello.txt";
    const data = encoder.encode("Hello world!\n");

    using file = await Deno.open(filename, {
      write: true,
      truncate: true,
      create: true,
      read: true,
    });
    const seekPosition = 0;
    // assert file was created
    let fileInfo = Deno.statSync(filename);
    assert(fileInfo.isFile);
    assertEquals(fileInfo.size, 0);
    // write some data
    await file.write(data);
    fileInfo = Deno.statSync(filename);
    assertEquals(fileInfo.size, 13);

    const buf = new Uint8Array(20);
    // seeking from beginning of a file
    const cursorPosition = await file.seek(seekPosition, Deno.SeekMode.Start);
    assertEquals(seekPosition, cursorPosition);
    const result = await file.read(buf);
    assertEquals(result, 13);

    await Deno.remove(tempDir, { recursive: true });
  },
);

Deno.test({ permissions: { read: true } }, async function seekStart() {
  const filename = "tests/testdata/assets/hello.txt";
  using file = await Deno.open(filename);
  const seekPosition = 6;
  // Deliberately move 1 step forward
  await file.read(new Uint8Array(1)); // "H"
  // Skipping "Hello "
  // seeking from beginning of a file plus seekPosition
  const cursorPosition = await file.seek(seekPosition, Deno.SeekMode.Start);
  assertEquals(seekPosition, cursorPosition);
  const buf = new Uint8Array(6);
  await file.read(buf);
  const decoded = new TextDecoder().decode(buf);
  assertEquals(decoded, "world!");
});

Deno.test({ permissions: { read: true } }, async function seekStartBigInt() {
  const filename = "tests/testdata/assets/hello.txt";
  using file = await Deno.open(filename);
  const seekPosition = 6n;
  // Deliberately move 1 step forward
  await file.read(new Uint8Array(1)); // "H"
  // Skipping "Hello "
  // seeking from beginning of a file plus seekPosition
  const cursorPosition = await file.seek(seekPosition, Deno.SeekMode.Start);
  assertEquals(seekPosition, BigInt(cursorPosition));
  const buf = new Uint8Array(6);
  await file.read(buf);
  const decoded = new TextDecoder().decode(buf);
  assertEquals(decoded, "world!");
});

Deno.test({ permissions: { read: true } }, function seekSyncStart() {
  const filename = "tests/testdata/assets/hello.txt";
  using file = Deno.openSync(filename);
  const seekPosition = 6;
  // Deliberately move 1 step forward
  file.readSync(new Uint8Array(1)); // "H"
  // Skipping "Hello "
  // seeking from beginning of a file plus seekPosition
  const cursorPosition = file.seekSync(seekPosition, Deno.SeekMode.Start);
  assertEquals(seekPosition, cursorPosition);
  const buf = new Uint8Array(6);
  file.readSync(buf);
  const decoded = new TextDecoder().decode(buf);
  assertEquals(decoded, "world!");
});

Deno.test({ permissions: { read: true } }, async function seekCurrent() {
  const filename = "tests/testdata/assets/hello.txt";
  using file = await Deno.open(filename);
  // Deliberately move 1 step forward
  await file.read(new Uint8Array(1)); // "H"
  // Skipping "ello "
  const seekPosition = 5;
  // seekPosition is relative to current cursor position after read
  const cursorPosition = await file.seek(seekPosition, Deno.SeekMode.Current);
  assertEquals(seekPosition + 1, cursorPosition);
  const buf = new Uint8Array(6);
  await file.read(buf);
  const decoded = new TextDecoder().decode(buf);
  assertEquals(decoded, "world!");
});

Deno.test({ permissions: { read: true } }, function seekSyncCurrent() {
  const filename = "tests/testdata/assets/hello.txt";
  using file = Deno.openSync(filename);
  // Deliberately move 1 step forward
  file.readSync(new Uint8Array(1)); // "H"
  // Skipping "ello "
  const seekPosition = 5;
  // seekPosition is relative to current cursor position after read
  const cursorPosition = file.seekSync(seekPosition, Deno.SeekMode.Current);
  assertEquals(seekPosition + 1, cursorPosition);
  const buf = new Uint8Array(6);
  file.readSync(buf);
  const decoded = new TextDecoder().decode(buf);
  assertEquals(decoded, "world!");
});

Deno.test({ permissions: { read: true } }, async function seekEnd() {
  const filename = "tests/testdata/assets/hello.txt";
  using file = await Deno.open(filename);
  const seekPosition = -6;
  // seek from end of file that has 12 chars, 12 - 6  = 6
  const cursorPosition = await file.seek(seekPosition, Deno.SeekMode.End);
  assertEquals(6, cursorPosition);
  const buf = new Uint8Array(6);
  await file.read(buf);
  const decoded = new TextDecoder().decode(buf);
  assertEquals(decoded, "world!");
});

Deno.test({ permissions: { read: true } }, function seekSyncEnd() {
  const filename = "tests/testdata/assets/hello.txt";
  using file = Deno.openSync(filename);
  const seekPosition = -6;
  // seek from end of file that has 12 chars, 12 - 6  = 6
  const cursorPosition = file.seekSync(seekPosition, Deno.SeekMode.End);
  assertEquals(6, cursorPosition);
  const buf = new Uint8Array(6);
  file.readSync(buf);
  const decoded = new TextDecoder().decode(buf);
  assertEquals(decoded, "world!");
});

Deno.test({ permissions: { read: true } }, async function seekMode() {
  const filename = "tests/testdata/assets/hello.txt";
  using file = await Deno.open(filename);
  await assertRejects(
    async () => {
      await file.seek(1, -1 as unknown as Deno.SeekMode);
    },
    TypeError,
    "Invalid seek mode",
  );

  // We should still be able to read the file
  // since it is still open.
  const buf = new Uint8Array(1);
  await file.read(buf); // "H"
  assertEquals(new TextDecoder().decode(buf), "H");
});

Deno.test(
  { permissions: { read: true, write: true } },
  function fileTruncateSyncSuccess() {
    const filename = Deno.makeTempDirSync() + "/test_fileTruncateSync.txt";
    using file = Deno.openSync(filename, {
      create: true,
      read: true,
      write: true,
    });

    file.truncateSync(20);
    assertEquals(Deno.readFileSync(filename).byteLength, 20);
    file.truncateSync(5);
    assertEquals(Deno.readFileSync(filename).byteLength, 5);
    file.truncateSync(-5);
    assertEquals(Deno.readFileSync(filename).byteLength, 0);

    Deno.removeSync(filename);
  },
);

Deno.test(
  { permissions: { read: true, write: true } },
  async function fileTruncateSuccess() {
    const filename = Deno.makeTempDirSync() + "/test_fileTruncate.txt";
    using file = await Deno.open(filename, {
      create: true,
      read: true,
      write: true,
    });

    await file.truncate(20);
    assertEquals((await Deno.readFile(filename)).byteLength, 20);
    await file.truncate(5);
    assertEquals((await Deno.readFile(filename)).byteLength, 5);
    await file.truncate(-5);
    assertEquals((await Deno.readFile(filename)).byteLength, 0);

    await Deno.remove(filename);
  },
);

Deno.test({ permissions: { read: true } }, function fileStatSyncSuccess() {
  using file = Deno.openSync("README.md");
  const fileInfo = file.statSync();
  assert(fileInfo.isFile);
  assert(!fileInfo.isSymlink);
  assert(!fileInfo.isDirectory);
  assert(fileInfo.size);
  assert(fileInfo.atime);
  assert(fileInfo.mtime);
  // The `birthtime` field is not available on Linux before kernel version 4.11.
  assert(fileInfo.birthtime || Deno.build.os === "linux");
});

Deno.test(async function fileStatSuccess() {
  using file = await Deno.open("README.md");
  const fileInfo = await file.stat();
  assert(fileInfo.isFile);
  assert(!fileInfo.isSymlink);
  assert(!fileInfo.isDirectory);
  assert(fileInfo.size);
  assert(fileInfo.atime);
  assert(fileInfo.mtime);
  // The `birthtime` field is not available on Linux before kernel version 4.11.
  assert(fileInfo.birthtime || Deno.build.os === "linux");
});

Deno.test({ permissions: { read: true } }, async function readableStream() {
  const filename = "tests/testdata/assets/hello.txt";
  const file = await Deno.open(filename);
  assert(file.readable instanceof ReadableStream);
  const chunks = [];
  for await (const chunk of file.readable) {
    chunks.push(chunk);
  }
  assertEquals(chunks.length, 1);
  assertEquals(chunks[0].byteLength, 12);
});

Deno.test(
  { permissions: { read: true } },
  async function readableStreamTextEncoderPipe() {
    const filename = "tests/testdata/assets/hello.txt";
    const file = await Deno.open(filename);
    const readable = file.readable.pipeThrough(new TextDecoderStream());
    const chunks = [];
    for await (const chunk of readable) {
      chunks.push(chunk);
    }
    assertEquals(chunks.length, 1);
    assertEquals(chunks[0].length, 12);
  },
);

Deno.test(
  { permissions: { read: true, write: true } },
  async function writableStream() {
    const path = await Deno.makeTempFile();
    const file = await Deno.open(path, { write: true });
    assert(file.writable instanceof WritableStream);
    const readable = new ReadableStream({
      start(controller) {
        controller.enqueue(new TextEncoder().encode("hello "));
        controller.enqueue(new TextEncoder().encode("world!"));
        controller.close();
      },
    });
    await readable.pipeTo(file.writable);
    const res = await Deno.readTextFile(path);
    assertEquals(res, "hello world!");
  },
);

Deno.test(
  { permissions: { read: true, write: true } },
  async function readTextFileNonUtf8() {
    const path = await Deno.makeTempFile();
    using file = await Deno.open(path, { write: true });
    await file.write(new TextEncoder().encode("hello "));
    await file.write(new Uint8Array([0xC0]));

    const res = await Deno.readTextFile(path);
    const resSync = Deno.readTextFileSync(path);
    assertEquals(res, resSync);
    assertEquals(res, "hello \uFFFD");
  },
);

Deno.test(
  { permissions: { read: true } },
  async function fsFileExplicitResourceManagement() {
    let file2: Deno.FsFile;

    {
      using file = await Deno.open("tests/testdata/assets/hello.txt");
      file2 = file;

      const stat = file.statSync();
      assert(stat.isFile);
    }

    assertThrows(() => file2.statSync(), Deno.errors.BadResource);
  },
);

Deno.test(
  { permissions: { read: true } },
  async function fsFileExplicitResourceManagementManualClose() {
    using file = await Deno.open("tests/testdata/assets/hello.txt");
    file.close();
    assertThrows(() => file.statSync(), Deno.errors.BadResource); // definitely closed
    // calling [Symbol.dispose] after manual close is a no-op
  },
);

Deno.test(
  { permissions: { read: true, write: true } },
  function fsFileDatasyncSyncSuccess() {
    const filename = Deno.makeTempDirSync() + "/test_fdatasyncSync.txt";
    const file = Deno.openSync(filename, {
      read: true,
      write: true,
      create: true,
    });
    const data = new Uint8Array(64);
    file.writeSync(data);
    file.syncDataSync();
    assertEquals(Deno.readFileSync(filename), data);
    file.close();
    Deno.removeSync(filename);
  },
);

Deno.test(
  { permissions: { read: true, write: true } },
  async function fsFileDatasyncSuccess() {
    const filename = (await Deno.makeTempDir()) + "/test_fdatasync.txt";
    const file = await Deno.open(filename, {
      read: true,
      write: true,
      create: true,
    });
    const data = new Uint8Array(64);
    await file.write(data);
    await file.syncData();
    assertEquals(await Deno.readFile(filename), data);
    file.close();
    await Deno.remove(filename);
  },
);

Deno.test(
  { permissions: { read: true, write: true } },
  function fsFileSyncSyncSuccess() {
    const filename = Deno.makeTempDirSync() + "/test_fsyncSync.txt";
    const file = Deno.openSync(filename, {
      read: true,
      write: true,
      create: true,
    });
    const size = 64;
    file.truncateSync(size);
    file.syncSync();
    assertEquals(file.statSync().size, size);
    file.close();
    Deno.removeSync(filename);
  },
);

Deno.test(
  { permissions: { read: true, write: true } },
  async function fsFileSyncSuccess() {
    const filename = (await Deno.makeTempDir()) + "/test_fsync.txt";
    const file = await Deno.open(filename, {
      read: true,
      write: true,
      create: true,
    });
    const size = 64;
    await file.truncate(size);
    await file.sync();
    assertEquals((await file.stat()).size, size);
    file.close();
    await Deno.remove(filename);
  },
);

Deno.test({ permissions: { read: true } }, function fsFileIsTerminal() {
  // CI not under TTY, so cannot test stdin/stdout/stderr.
  using file = Deno.openSync("tests/testdata/assets/hello.txt");
  assert(!file.isTerminal());
});

Deno.test(
  { permissions: { read: true, run: true } },
  async function fsFileLockFileSync() {
    await runFlockTests({ sync: true });
  },
);

Deno.test(
  { permissions: { read: true, run: true } },
  async function fsFileLockFileAsync() {
    await runFlockTests({ sync: false });
  },
);

async function runFlockTests(opts: { sync: boolean }) {
  assertEquals(
    await checkFirstBlocksSecond({
      firstExclusive: true,
      secondExclusive: false,
      sync: opts.sync,
    }),
    true,
    "exclusive blocks shared",
  );
  assertEquals(
    await checkFirstBlocksSecond({
      firstExclusive: false,
      secondExclusive: true,
      sync: opts.sync,
    }),
    true,
    "shared blocks exclusive",
  );
  assertEquals(
    await checkFirstBlocksSecond({
      firstExclusive: true,
      secondExclusive: true,
      sync: opts.sync,
    }),
    true,
    "exclusive blocks exclusive",
  );
  assertEquals(
    await checkFirstBlocksSecond({
      firstExclusive: false,
      secondExclusive: false,
      sync: opts.sync,
      // need to wait for both to enter the lock to prevent the case where the
      // first process enters and exits the lock before the second even enters
      waitBothEnteredLock: true,
    }),
    false,
    "shared does not block shared",
  );
}

async function checkFirstBlocksSecond(opts: {
  firstExclusive: boolean;
  secondExclusive: boolean;
  sync: boolean;
  waitBothEnteredLock?: boolean;
}) {
  const firstProcess = runFlockTestProcess({
    exclusive: opts.firstExclusive,
    sync: opts.sync,
  });
  const secondProcess = runFlockTestProcess({
    exclusive: opts.secondExclusive,
    sync: opts.sync,
  });
  try {
    const sleep = (time: number) => new Promise((r) => setTimeout(r, time));

    await Promise.all([
      firstProcess.waitStartup(),
      secondProcess.waitStartup(),
    ]);

    await firstProcess.enterLock();
    await firstProcess.waitEnterLock();

    await secondProcess.enterLock();
    await sleep(100);

    if (!opts.waitBothEnteredLock) {
      await firstProcess.exitLock();
    }

    await secondProcess.waitEnterLock();

    if (opts.waitBothEnteredLock) {
      await firstProcess.exitLock();
    }

    await secondProcess.exitLock();

    // collect the final output
    const firstPsTimes = await firstProcess.getTimes();
    const secondPsTimes = await secondProcess.getTimes();
    return firstPsTimes.exitTime < secondPsTimes.enterTime;
  } finally {
    await firstProcess.close();
    await secondProcess.close();
  }
}

function runFlockTestProcess(opts: { exclusive: boolean; sync: boolean }) {
  const path = "tests/testdata/assets/lock_target.txt";
  const scriptText = `
    const file = Deno.openSync("${path}");

    // ready signal
    Deno.stdout.writeSync(new Uint8Array(1));
    // wait for enter lock signal
    Deno.stdin.readSync(new Uint8Array(1));

    // entering signal
    Deno.stdout.writeSync(new Uint8Array(1));
    // lock and record the entry time
    ${
    opts.sync
      ? `file.lockSync(${opts.exclusive ? "true" : "false"});`
      : `await file.lock(${opts.exclusive ? "true" : "false"});`
  }
    const enterTime = new Date().getTime();
    // entered signal
    Deno.stdout.writeSync(new Uint8Array(1));

    // wait for exit lock signal
    Deno.stdin.readSync(new Uint8Array(1));

    // record the exit time and wait a little bit before releasing
    // the lock so that the enter time of the next process doesn't
    // occur at the same time as this exit time
    const exitTime = new Date().getTime();
    await new Promise(resolve => setTimeout(resolve, 100));

    // release the lock
    ${opts.sync ? "file.unlockSync();" : "await file.unlock();"}

    // exited signal
    Deno.stdout.writeSync(new Uint8Array(1));

    // output the enter and exit time
    console.log(JSON.stringify({ enterTime, exitTime }));
`;

  const process = new Deno.Command(Deno.execPath(), {
    args: ["eval", scriptText],
    stdin: "piped",
    stdout: "piped",
    stderr: "null",
  }).spawn();

  const waitSignal = async () => {
    const reader = process.stdout.getReader({ mode: "byob" });
    await reader.read(new Uint8Array(1));
    reader.releaseLock();
  };
  const signal = async () => {
    const writer = process.stdin.getWriter();
    await writer.write(new Uint8Array(1));
    writer.releaseLock();
  };

  return {
    async waitStartup() {
      await waitSignal();
    },
    async enterLock() {
      await signal();
      await waitSignal(); // entering signal
    },
    async waitEnterLock() {
      await waitSignal();
    },
    async exitLock() {
      await signal();
      await waitSignal();
    },
    getTimes: async () => {
      const { stdout } = await process.output();
      const text = new TextDecoder().decode(stdout);
      return JSON.parse(text) as {
        enterTime: number;
        exitTime: number;
      };
    },
    close: async () => {
      await process.status;
      await process.stdin.close();
    },
  };
}
