// Copyright 2018-2026 the Deno authors. MIT license.
// deno-lint-ignore-file no-explicit-any
import vfs, {
  create,
  MemoryFileHandle,
  MemoryProvider,
  RealFSProvider,
  VirtualDir,
  VirtualFileHandle,
  VirtualFileSystem,
  VirtualProvider,
} from "node:vfs";
import {
  assert,
  assertEquals,
  assertInstanceOf,
  assertRejects,
  assertStrictEquals,
  assertThrows,
} from "@std/assert";
import { Buffer } from "node:buffer";
import { Stats } from "node:fs";
import * as path from "node:path";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import * as os from "node:os";

const noWarn = { emitExperimentalWarning: false } as const;

Deno.test("[node/vfs] default and named exports match", () => {
  assertStrictEquals(vfs.create, create);
  assertStrictEquals(vfs.VirtualFileSystem, VirtualFileSystem);
  assertStrictEquals(vfs.MemoryProvider, MemoryProvider);
  assertStrictEquals(vfs.RealFSProvider, RealFSProvider);
  assertStrictEquals(vfs.VirtualProvider, VirtualProvider);
  assertStrictEquals(vfs.VirtualFileHandle, VirtualFileHandle);
  assertStrictEquals(vfs.MemoryFileHandle, MemoryFileHandle);
  assertStrictEquals(vfs.VirtualDir, VirtualDir);
});

Deno.test("[node/vfs] create() returns a VirtualFileSystem with MemoryProvider", () => {
  const fs = create(noWarn);
  assertInstanceOf(fs, VirtualFileSystem);
  assertInstanceOf(fs.provider, MemoryProvider);
  assertEquals(fs.readonly, false);
});

Deno.test("[node/vfs] writeFileSync + readFileSync round-trip", () => {
  const fs = create(noWarn);
  fs.writeFileSync("/hello.txt", "world");
  const buf = fs.readFileSync("/hello.txt");
  assertInstanceOf(buf, Buffer);
  assertEquals(buf.toString(), "world");
  assertEquals(fs.readFileSync("/hello.txt", "utf8"), "world");
});

Deno.test("[node/vfs] statSync returns a real Stats instance", () => {
  const fs = create(noWarn);
  fs.writeFileSync("/file.txt", "hello");
  const st = fs.statSync("/file.txt");
  assertInstanceOf(st, Stats);
  assertEquals(st.isFile(), true);
  assertEquals(st.isDirectory(), false);
  assertEquals(st.size, 5);
});

Deno.test("[node/vfs] mkdirSync recursive + readdirSync", () => {
  const fs = create(noWarn);
  fs.mkdirSync("/a/b/c", { recursive: true });
  fs.writeFileSync("/a/b/c/file.txt", "1");
  fs.writeFileSync("/a/b/c/file2.txt", "2");
  const names = fs.readdirSync("/a/b/c") as string[];
  assertEquals(names.sort(), ["file.txt", "file2.txt"]);
});

Deno.test("[node/vfs] readdirSync withFileTypes returns Dirent entries", () => {
  const fs = create(noWarn);
  fs.mkdirSync("/dir");
  fs.writeFileSync("/dir/inner.txt", "hi");
  fs.mkdirSync("/dir/nested");
  const entries = fs.readdirSync("/dir", { withFileTypes: true }) as any[];
  assertEquals(entries.length, 2);
  const byName: Record<string, any> = Object.fromEntries(
    entries.map((e) => [e.name, e]),
  );
  assert(byName["inner.txt"].isFile());
  assert(byName["nested"].isDirectory());
  assertEquals(byName["inner.txt"].parentPath, "/dir");
});

Deno.test("[node/vfs] readFileSync on missing throws ENOENT", () => {
  const fs = create(noWarn);
  const err = assertThrows(() => fs.readFileSync("/no.txt"));
  assertEquals((err as any).code, "ENOENT");
});

Deno.test("[node/vfs] mkdirSync on existing without recursive throws EEXIST", () => {
  const fs = create(noWarn);
  fs.mkdirSync("/dup");
  const err = assertThrows(() => fs.mkdirSync("/dup"));
  assertEquals((err as any).code, "EEXIST");
});

Deno.test("[node/vfs] readonly provider rejects writes with EROFS", () => {
  const provider = new MemoryProvider();
  const fs = create(provider, noWarn);
  fs.writeFileSync("/a.txt", "data");
  provider.setReadOnly();
  const err = assertThrows(() => fs.writeFileSync("/b.txt", "data"));
  assertEquals((err as any).code, "EROFS");
  assertEquals(fs.readFileSync("/a.txt", "utf8"), "data");
});

Deno.test("[node/vfs] appendFileSync concatenates", () => {
  const fs = create(noWarn);
  fs.writeFileSync("/log.txt", "a");
  fs.appendFileSync("/log.txt", "b");
  fs.appendFileSync("/log.txt", "c");
  assertEquals(fs.readFileSync("/log.txt", "utf8"), "abc");
});

Deno.test("[node/vfs] renameSync moves a file", () => {
  const fs = create(noWarn);
  fs.writeFileSync("/src.txt", "v");
  fs.renameSync("/src.txt", "/dest.txt");
  assert(!fs.existsSync("/src.txt"));
  assertEquals(fs.readFileSync("/dest.txt", "utf8"), "v");
});

Deno.test("[node/vfs] copyFileSync copies content", () => {
  const fs = create(noWarn);
  fs.writeFileSync("/orig.txt", "payload");
  fs.copyFileSync("/orig.txt", "/copy.txt");
  assertEquals(fs.readFileSync("/copy.txt", "utf8"), "payload");
  fs.writeFileSync("/copy.txt", "changed");
  assertEquals(fs.readFileSync("/orig.txt", "utf8"), "payload");
});

Deno.test("[node/vfs] symlinkSync + readlinkSync + lstatSync", () => {
  const fs = create(noWarn);
  fs.writeFileSync("/target.txt", "real");
  fs.symlinkSync("/target.txt", "/link.txt");
  assertEquals(fs.readlinkSync("/link.txt"), "/target.txt");
  const lst = fs.lstatSync("/link.txt");
  assertEquals(lst.isSymbolicLink(), true);
  const st = fs.statSync("/link.txt");
  assertEquals(st.isFile(), true);
  assertEquals(fs.readFileSync("/link.txt", "utf8"), "real");
});

Deno.test("[node/vfs] open/read/close fd round-trip", () => {
  const fs = create(noWarn);
  fs.writeFileSync("/fd.txt", "abcdef");
  const fd = fs.openSync("/fd.txt");
  const buf = Buffer.alloc(4);
  const n = fs.readSync(fd, buf, 0, 4, 0);
  assertEquals(n, 4);
  assertEquals(buf.toString(), "abcd");
  const stat = fs.fstatSync(fd);
  assertEquals(stat.size, 6);
  fs.closeSync(fd);
});

Deno.test("[node/vfs] writeSync writes through an fd", () => {
  const fs = create(noWarn);
  const fd = fs.openSync("/w.txt", "w");
  const buf = Buffer.from("hello fd");
  fs.writeSync(fd, buf, 0, buf.length, null);
  fs.closeSync(fd);
  assertEquals(fs.readFileSync("/w.txt", "utf8"), "hello fd");
});

Deno.test("[node/vfs] callback API readFile/writeFile", async () => {
  const fs = create(noWarn);
  await new Promise<void>((resolve, reject) => {
    fs.writeFile("/cb.txt", "cb-data", (err: Error | null) => {
      if (err) return reject(err);
      resolve();
    });
  });
  const data = await new Promise<string>((resolve, reject) => {
    fs.readFile(
      "/cb.txt",
      "utf8",
      (err: Error | null, data: string | Buffer) => {
        if (err) return reject(err);
        resolve(data as string);
      },
    );
  });
  assertEquals(data, "cb-data");
});

Deno.test("[node/vfs] promises API readFile/writeFile/mkdir/readdir", async () => {
  const fs = create(noWarn);
  await fs.promises.mkdir("/p");
  await fs.promises.writeFile("/p/x.txt", "promise");
  assertEquals(await fs.promises.readFile("/p/x.txt", "utf8"), "promise");
  const names = await fs.promises.readdir("/p");
  assertEquals(names, ["x.txt"]);
  const stats = await fs.promises.stat("/p/x.txt");
  assertEquals(stats.isFile(), true);
});

Deno.test("[node/vfs] promises API rejects missing file", async () => {
  const fs = create(noWarn);
  await assertRejects(() => fs.promises.readFile("/nope"));
});

Deno.test("[node/vfs] promises.rm with recursive", async () => {
  const fs = create(noWarn);
  await fs.promises.mkdir("/a/b", { recursive: true });
  await fs.promises.writeFile("/a/b/f.txt", "x");
  await fs.promises.rm("/a", { recursive: true });
  assertEquals(fs.existsSync("/a"), false);
});

Deno.test("[node/vfs] linkSync creates a hard link sharing content", () => {
  const fs = create(noWarn);
  fs.writeFileSync("/h.txt", "shared");
  fs.linkSync("/h.txt", "/h2.txt");
  assertEquals(fs.readFileSync("/h2.txt", "utf8"), "shared");
  const fd = fs.openSync("/h.txt", "r+");
  const buf = Buffer.from("rewrote");
  fs.writeSync(fd, buf, 0, buf.length, 0);
  fs.closeSync(fd);
  assertEquals(fs.readFileSync("/h2.txt", "utf8"), "rewrote");
});

Deno.test("[node/vfs] truncateSync truncates content", () => {
  const fs = create(noWarn);
  fs.writeFileSync("/t.txt", "hello world");
  fs.truncateSync("/t.txt", 5);
  assertEquals(fs.readFileSync("/t.txt", "utf8"), "hello");
});

Deno.test("[node/vfs] chmodSync updates mode bits", () => {
  const fs = create(noWarn);
  fs.writeFileSync("/m.txt", "x");
  fs.chmodSync("/m.txt", 0o600);
  const st = fs.statSync("/m.txt");
  assertEquals(st.mode & 0o777, 0o600);
});

Deno.test("[node/vfs] mkdtempSync creates a unique directory", () => {
  const fs = create(noWarn);
  const dir = fs.mkdtempSync("/tmp-");
  assert(dir.startsWith("/tmp-"));
  assertEquals(dir.length, "/tmp-".length + 6);
  assertEquals(fs.statSync(dir).isDirectory(), true);
});

Deno.test("[node/vfs] opendirSync iterates entries", async () => {
  const fs = create(noWarn);
  fs.mkdirSync("/d");
  fs.writeFileSync("/d/a", "1");
  fs.writeFileSync("/d/b", "2");
  const dir = fs.opendirSync("/d");
  assertInstanceOf(dir, VirtualDir);
  const seen: string[] = [];
  for await (const entry of dir as any) {
    seen.push((entry as any).name);
  }
  assertEquals(seen.sort(), ["a", "b"]);
});

Deno.test("[node/vfs] openAsBlob returns a Blob with file content", async () => {
  const fs = create(noWarn);
  fs.writeFileSync("/blob.txt", "blob-data");
  const blob = fs.openAsBlob("/blob.txt");
  assertInstanceOf(blob, Blob);
  assertEquals(await blob.text(), "blob-data");
});

Deno.test("[node/vfs] custom provider extending VirtualProvider", () => {
  const store = new Map<string, Buffer>();
  class CustomProvider extends VirtualProvider {
    get readonly() {
      return false;
    }
    statSync(p: string) {
      const c = store.get(p);
      if (!c) {
        const e = new Error("ENOENT") as any;
        e.code = "ENOENT";
        throw e;
      }
      // Return a fake stats-like object - the VFS forwards it through unchanged.
      return {
        size: c.length,
        mode: 0o644 | 0o100000,
        isFile: () => true,
        isDirectory: () => false,
        isSymbolicLink: () => false,
      } as any;
    }
    readFileSync(p: string) {
      const c = store.get(p);
      if (!c) {
        const e = new Error("ENOENT") as any;
        e.code = "ENOENT";
        throw e;
      }
      return Buffer.from(c);
    }
    writeFileSync(p: string, data: any) {
      const buf = typeof data === "string" ? Buffer.from(data) : data;
      store.set(p, Buffer.from(buf));
    }
  }
  const fs = create(new CustomProvider(), noWarn);
  fs.writeFileSync("/key", "value");
  assertEquals(fs.readFileSync("/key").toString(), "value");
  assertEquals(fs.existsSync("/key"), true);
  assertEquals(fs.existsSync("/missing"), false);
});

Deno.test("[node/vfs] createReadStream emits the file", async () => {
  const fs = create(noWarn);
  fs.writeFileSync("/stream.txt", "stream-payload");
  const stream = fs.createReadStream("/stream.txt");
  const chunks: Buffer[] = [];
  for await (const chunk of stream as any) {
    chunks.push(chunk as Buffer);
  }
  assertEquals(Buffer.concat(chunks).toString(), "stream-payload");
});

Deno.test("[node/vfs] createWriteStream writes the file", async () => {
  const fs = create(noWarn);
  const stream = fs.createWriteStream("/written.txt");
  await new Promise<void>((resolve, reject) => {
    stream.on("finish", () => resolve());
    stream.on("error", reject);
    stream.write("first ");
    stream.write("second");
    stream.end();
  });
  assertEquals(fs.readFileSync("/written.txt", "utf8"), "first second");
});

Deno.test("[node/vfs] RealFSProvider exposes a real directory", () => {
  const tmp = mkdtempSync(path.join(os.tmpdir(), "deno-vfs-real-"));
  try {
    writeFileSync(path.join(tmp, "seed.txt"), "hello-real");
    const provider = new RealFSProvider(tmp);
    assertEquals(provider.rootPath, path.resolve(tmp));
    const fs = create(provider, noWarn);
    assertEquals(fs.readFileSync("/seed.txt", "utf8"), "hello-real");
    fs.writeFileSync("/written.txt", "from vfs");
    assertEquals(fs.readFileSync("/written.txt", "utf8"), "from vfs");
    // Escape attempts blocked
    assertThrows(() => fs.readFileSync("/../escape"));
  } finally {
    rmSync(tmp, { recursive: true, force: true });
  }
});

Deno.test("[node/vfs] watcher emits change events for in-memory writes", async () => {
  const fs = create(noWarn);
  fs.writeFileSync("/watched.txt", "v1");
  const watcher = fs.watch("/watched.txt", { interval: 20 }) as any;
  const seen = new Promise<{ type: string; filename: string }>((resolve) => {
    watcher.once("change", (type: string, filename: string) => {
      resolve({ type, filename });
    });
  });
  // Mutate after the watcher latches its baseline stats.
  await new Promise((r) => setTimeout(r, 25));
  fs.writeFileSync("/watched.txt", "v2-different-length");
  const event = await seen;
  assertEquals(event.type, "change");
  assertEquals(event.filename, "watched.txt");
  watcher.close();
});

Deno.test("[node/vfs] promises.watch is cancellable via AbortSignal", async () => {
  const fs = create(noWarn);
  fs.writeFileSync("/cancel.txt", "v1");
  const ac = new AbortController();
  const watcher = fs.promises.watch("/cancel.txt", {
    interval: 20,
    signal: ac.signal,
  }) as AsyncIterable<unknown>;
  setTimeout(() => ac.abort(), 30);
  await assertRejects(async () => {
    for await (const _ of watcher) {
      // ignore events
    }
  });
});
