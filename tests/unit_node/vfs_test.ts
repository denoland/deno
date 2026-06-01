// Copyright 2018-2026 the Deno authors. MIT license.
// deno-lint-ignore-file require-await no-explicit-any
import vfs, {
  create,
  MemoryProvider,
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

Deno.test("[node/vfs] default and named exports match", () => {
  assertStrictEquals(vfs.create, create);
  assertStrictEquals(vfs.VirtualFileSystem, VirtualFileSystem);
  assertStrictEquals(vfs.MemoryProvider, MemoryProvider);
  assertStrictEquals(vfs.VirtualProvider, VirtualProvider);
});

Deno.test("[node/vfs] create() returns a VirtualFileSystem", () => {
  const fs = create();
  assertInstanceOf(fs, VirtualFileSystem);
  assertInstanceOf(fs.provider, MemoryProvider);
  assertEquals(fs.mounted, false);
  assertStrictEquals(fs.mountPoint, null);
  assertEquals(fs.readonly, false);
  assertEquals(fs.overlay, false);
});

Deno.test("[node/vfs] writeFileSync + readFileSync round-trip", () => {
  const fs = create();
  fs.writeFileSync("/hello.txt", "world");
  const buf = fs.readFileSync("/hello.txt");
  assertInstanceOf(buf, Buffer);
  assertEquals(buf.toString(), "world");
  assertEquals(fs.readFileSync("/hello.txt", "utf8"), "world");
});

Deno.test("[node/vfs] existsSync reflects file presence", () => {
  const fs = create();
  assertEquals(fs.existsSync("/missing.txt"), false);
  fs.writeFileSync("/present.txt", "x");
  assertEquals(fs.existsSync("/present.txt"), true);
});

Deno.test("[node/vfs] statSync of a file", () => {
  const fs = create();
  fs.writeFileSync("/file.txt", "hello");
  const st = fs.statSync("/file.txt");
  assertEquals(st.isFile(), true);
  assertEquals(st.isDirectory(), false);
  assertEquals(st.size, 5);
});

Deno.test("[node/vfs] mkdirSync recursive + readdirSync", () => {
  const fs = create();
  fs.mkdirSync("/a/b/c", { recursive: true });
  fs.writeFileSync("/a/b/c/file.txt", "1");
  fs.writeFileSync("/a/b/c/file2.txt", "2");
  const names = fs.readdirSync("/a/b/c");
  assertEquals(names.sort(), ["file.txt", "file2.txt"]);
});

Deno.test("[node/vfs] readdirSync withFileTypes returns Dirent-like entries", () => {
  const fs = create();
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
});

Deno.test("[node/vfs] unlinkSync removes a file", () => {
  const fs = create();
  fs.writeFileSync("/del.txt", "x");
  assert(fs.existsSync("/del.txt"));
  fs.unlinkSync("/del.txt");
  assert(!fs.existsSync("/del.txt"));
});

Deno.test("[node/vfs] readFileSync on missing throws ENOENT", () => {
  const fs = create();
  const err = assertThrows(() => fs.readFileSync("/no.txt"));
  assertEquals((err as any).code, "ENOENT");
});

Deno.test("[node/vfs] mkdirSync on existing without recursive throws EEXIST", () => {
  const fs = create();
  fs.mkdirSync("/dup");
  const err = assertThrows(() => fs.mkdirSync("/dup"));
  assertEquals((err as any).code, "EEXIST");
});

Deno.test("[node/vfs] readonly provider rejects writes with EROFS", () => {
  const provider = new MemoryProvider();
  const fs = create(provider);
  fs.writeFileSync("/a.txt", "data");
  provider.setReadOnly();
  const err = assertThrows(() => fs.writeFileSync("/b.txt", "data"));
  assertEquals((err as any).code, "EROFS");
  // Read still works.
  assertEquals(fs.readFileSync("/a.txt", "utf8"), "data");
});

Deno.test("[node/vfs] appendFileSync concatenates", () => {
  const fs = create();
  fs.writeFileSync("/log.txt", "a");
  fs.appendFileSync("/log.txt", "b");
  fs.appendFileSync("/log.txt", "c");
  assertEquals(fs.readFileSync("/log.txt", "utf8"), "abc");
});

Deno.test("[node/vfs] renameSync moves a file", () => {
  const fs = create();
  fs.writeFileSync("/src.txt", "v");
  fs.renameSync("/src.txt", "/dest.txt");
  assert(!fs.existsSync("/src.txt"));
  assertEquals(fs.readFileSync("/dest.txt", "utf8"), "v");
});

Deno.test("[node/vfs] copyFileSync copies content", () => {
  const fs = create();
  fs.writeFileSync("/orig.txt", "payload");
  fs.copyFileSync("/orig.txt", "/copy.txt");
  assertEquals(fs.readFileSync("/copy.txt", "utf8"), "payload");
  // Verify they are independent: editing copy doesn't change orig.
  fs.writeFileSync("/copy.txt", "changed");
  assertEquals(fs.readFileSync("/orig.txt", "utf8"), "payload");
});

Deno.test("[node/vfs] symlinkSync + readlinkSync + lstatSync", () => {
  const fs = create();
  fs.writeFileSync("/target.txt", "real");
  fs.symlinkSync("/target.txt", "/link.txt");
  assertEquals(fs.readlinkSync("/link.txt"), "/target.txt");
  const lst = fs.lstatSync("/link.txt");
  assertEquals(lst.isSymbolicLink(), true);
  // statSync follows the symlink
  const st = fs.statSync("/link.txt");
  assertEquals(st.isFile(), true);
  assertEquals(fs.readFileSync("/link.txt", "utf8"), "real");
});

Deno.test("[node/vfs] open/read/close fd round-trip", () => {
  const fs = create();
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

Deno.test("[node/vfs] mount + shouldHandle + unmount lifecycle", () => {
  const fs = create();
  assertEquals(fs.shouldHandle("/app/x"), false);
  fs.mount("/app");
  assertEquals(fs.mounted, true);
  assertEquals(fs.mountPoint, "/app");
  assertEquals(fs.shouldHandle("/app/x"), true);
  assertEquals(fs.shouldHandle("/elsewhere"), false);
  fs.writeFileSync("/app/file.txt", "mounted");
  assertEquals(fs.readFileSync("/app/file.txt", "utf8"), "mounted");
  fs.unmount();
  assertEquals(fs.mounted, false);
  assertStrictEquals(fs.mountPoint, null);
});

Deno.test("[node/vfs] mount errors when already mounted", () => {
  const fs = create();
  fs.mount("/x");
  assertThrows(() => fs.mount("/y"));
  fs.unmount();
});

Deno.test("[node/vfs] callback API readFile/writeFile", async () => {
  const fs = create();
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
  const fs = create();
  await fs.promises.mkdir("/p");
  await fs.promises.writeFile("/p/x.txt", "promise");
  assertEquals(await fs.promises.readFile("/p/x.txt", "utf8"), "promise");
  const names = await fs.promises.readdir("/p");
  assertEquals(names, ["x.txt"]);
  const stats = await fs.promises.stat("/p/x.txt");
  assertEquals(stats.isFile(), true);
});

Deno.test("[node/vfs] promises API rejects missing file", async () => {
  const fs = create();
  await assertRejects(() => fs.promises.readFile("/nope"));
});

Deno.test("[node/vfs] custom provider can extend VirtualProvider", () => {
  const store = new Map<string, Buffer>();
  class CustomProvider extends (VirtualProvider as any) {
    get readonly() {
      return false;
    }
    override openSync(path: string, flags: string, _mode?: number) {
      let content = store.get(path);
      if (!content) {
        if (flags === "r") {
          const err = new Error("ENOENT");
          (err as any).code = "ENOENT";
          throw err;
        }
        content = Buffer.alloc(0);
        store.set(path, content);
      }
      const self = this as any;
      return {
        path,
        flags,
        position: 0,
        closed: false,
        readFileSync(options?: string | { encoding?: string }) {
          const encoding = typeof options === "string"
            ? options
            : options?.encoding;
          return encoding
            ? content!.toString(encoding as BufferEncoding)
            : content;
        },
        writeFileSync(data: string | Buffer) {
          const buf = typeof data === "string" ? Buffer.from(data) : data;
          store.set(path, Buffer.from(buf));
          self.lastWritten = buf;
        },
        statSync() {
          return {
            size: content!.length,
            isFile: () => true,
            isDirectory: () => false,
            isSymbolicLink: () => false,
          };
        },
        closeSync() {},
        async readFile() {
          return content;
        },
        async writeFile(d: string | Buffer) {
          this.writeFileSync(d);
        },
        async stat() {
          return this.statSync();
        },
        async close() {},
      };
    }
    override async open(path: string, flags: string, mode?: number) {
      return this.openSync(path, flags, mode);
    }
    override statSync(path: string) {
      const content = store.get(path);
      if (!content) {
        const err = new Error("ENOENT");
        (err as any).code = "ENOENT";
        throw err;
      }
      return {
        size: content.length,
        mode: 0o644 | 0o100000,
        isFile: () => true,
        isDirectory: () => false,
        isSymbolicLink: () => false,
      };
    }
    override async stat(path: string) {
      return this.statSync(path);
    }
  }

  const fs = create(new CustomProvider());
  fs.writeFileSync("/key", "value");
  assertEquals(fs.readFileSync("/key", "utf8"), "value");
  assertEquals(fs.existsSync("/key"), true);
  assertEquals(fs.existsSync("/missing"), false);
});

Deno.test("[node/vfs] createReadStream emits the file", async () => {
  const fs = create();
  fs.writeFileSync("/stream.txt", "stream-payload");
  const stream = fs.createReadStream("/stream.txt");
  const chunks: Buffer[] = [];
  for await (const chunk of stream) {
    chunks.push(chunk as Buffer);
  }
  assertEquals(Buffer.concat(chunks).toString(), "stream-payload");
});

Deno.test("[node/vfs] virtualCwd: chdir + cwd round-trip", () => {
  const fs = create({ virtualCwd: true });
  fs.mkdirSync("/work", { recursive: true });
  fs.mount("/work");
  fs.chdir("/work");
  assertEquals(fs.cwd(), "/work");
  fs.unmount();
});

Deno.test("[node/vfs] cwd() throws when virtualCwd is disabled", () => {
  const fs = create();
  assertThrows(() => fs.cwd());
});

Deno.test("[node/vfs] internalModuleStat", () => {
  const fs = create();
  fs.mkdirSync("/somedir");
  fs.writeFileSync("/somedir/file.txt", "x");
  assertEquals(fs.internalModuleStat("/somedir"), 1);
  assertEquals(fs.internalModuleStat("/somedir/file.txt"), 0);
  assertEquals(fs.internalModuleStat("/does/not/exist"), -2);
});
