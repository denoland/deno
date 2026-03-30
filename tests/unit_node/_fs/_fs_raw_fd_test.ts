// Copyright 2018-2026 the Deno authors. MIT license.
//
// Tests that node:fs returns real OS file descriptors and that all
// fd-based operations work correctly on them.

import { assertEquals, assertThrows } from "@std/assert";
import * as fs from "node:fs";
import { Buffer } from "node:buffer";

// -- Helpers --

function withTempFile(
  content: string,
  fn: (path: string) => void,
) {
  const path = Deno.makeTempFileSync();
  Deno.writeTextFileSync(path, content);
  try {
    fn(path);
  } finally {
    try {
      Deno.removeSync(path);
    } catch {
      // ignore
    }
  }
}

async function withTempFileAsync(
  content: string,
  fn: (path: string) => Promise<void>,
) {
  const path = Deno.makeTempFileSync();
  Deno.writeTextFileSync(path, content);
  try {
    await fn(path);
  } finally {
    try {
      Deno.removeSync(path);
    } catch {
      // ignore
    }
  }
}

// -- Real OS fd tests --

Deno.test("[node/fs] openSync returns a real OS fd (not a small RID)", () => {
  withTempFile("test", (path) => {
    const fd = fs.openSync(path, "r");
    try {
      // Real OS fds are typically > 2 (0/1/2 are stdio).
      // Deno RIDs used to be small sequential integers (3, 4, 5...).
      // With real fds, the value should be a valid positive integer
      // assigned by the OS.
      assertEquals(typeof fd, "number");
      assertEquals(fd > 2, true);
    } finally {
      fs.closeSync(fd);
    }
  });
});

Deno.test("[node/fs] closeSync throws EBADF for invalid fd", () => {
  assertThrows(
    () => fs.closeSync(12345),
    Error,
  );
});

// -- Positioned read tests --

Deno.test("[node/fs] readSync with position reads at offset without moving cursor", () => {
  withTempFile("abcdefghij", (path) => {
    const fd = fs.openSync(path, "r");
    try {
      const buf1 = Buffer.alloc(3);
      // Read 3 bytes at position 4 ("efg")
      const n1 = fs.readSync(fd, buf1, 0, 3, 4);
      assertEquals(n1, 3);
      assertEquals(buf1.toString(), "efg");

      // Read from current position (should be 0, since positioned read
      // should NOT move the cursor)
      const buf2 = Buffer.alloc(3);
      const n2 = fs.readSync(fd, buf2, 0, 3, null);
      assertEquals(n2, 3);
      assertEquals(buf2.toString(), "abc");
    } finally {
      fs.closeSync(fd);
    }
  });
});

Deno.test("[node/fs] readSync sequential reads advance cursor", () => {
  withTempFile("hello world", (path) => {
    const fd = fs.openSync(path, "r");
    try {
      const buf1 = Buffer.alloc(5);
      fs.readSync(fd, buf1, 0, 5, null);
      assertEquals(buf1.toString(), "hello");

      const buf2 = Buffer.alloc(6);
      fs.readSync(fd, buf2, 0, 6, null);
      assertEquals(buf2.toString(), " world");
    } finally {
      fs.closeSync(fd);
    }
  });
});

Deno.test("[node/fs] async read with position does not move cursor", async () => {
  await withTempFileAsync("abcdefghij", async (path) => {
    const fd = fs.openSync(path, "r");
    try {
      const buf1 = Buffer.alloc(3);
      await new Promise<void>((resolve, reject) => {
        fs.read(fd, buf1, 0, 3, 5, (err, nread) => {
          if (err) return reject(err);
          assertEquals(nread, 3);
          assertEquals(buf1.toString(), "fgh");
          resolve();
        });
      });

      // Cursor should still be at 0
      const buf2 = Buffer.alloc(3);
      const n2 = fs.readSync(fd, buf2, 0, 3, null);
      assertEquals(n2, 3);
      assertEquals(buf2.toString(), "abc");
    } finally {
      fs.closeSync(fd);
    }
  });
});

// -- Positioned write tests --

Deno.test("[node/fs] writeSync with position writes at offset without moving cursor", () => {
  withTempFile("aaaaaaaaaa", (path) => {
    const fd = fs.openSync(path, "r+");
    try {
      // Write "XYZ" at position 3
      const n = fs.writeSync(fd, Buffer.from("XYZ"), 0, 3, 3);
      assertEquals(n, 3);

      // Cursor should still be at 0 (positioned write is pwrite-like)
      // Write "AB" at current position (0)
      fs.writeSync(fd, Buffer.from("AB"), 0, 2, null);
    } finally {
      fs.closeSync(fd);
    }
    assertEquals(Deno.readTextFileSync(path), "ABaXYZaaaa");
  });
});

Deno.test("[node/fs] async write with position does not move cursor", async () => {
  await withTempFileAsync("aaaaaaaaaa", async (path) => {
    const fd = fs.openSync(path, "r+");
    try {
      // Write "XY" at position 5
      await new Promise<void>((resolve, reject) => {
        fs.write(fd, Buffer.from("XY"), 0, 2, 5, (err) => {
          if (err) return reject(err);
          resolve();
        });
      });

      // Cursor should still be at 0
      fs.writeSync(fd, Buffer.from("ZZ"), 0, 2, null);
    } finally {
      fs.closeSync(fd);
    }
    assertEquals(Deno.readTextFileSync(path), "ZZaaaXYaaa");
  });
});

// -- fstat tests --

Deno.test("[node/fs] fstatSync returns correct info for fd", () => {
  withTempFile("hello world", (path) => {
    const fd = fs.openSync(path, "r");
    try {
      const stat = fs.fstatSync(fd);
      assertEquals(stat.isFile(), true);
      assertEquals(stat.isDirectory(), false);
      assertEquals(stat.size, 11);
    } finally {
      fs.closeSync(fd);
    }
  });
});

Deno.test("[node/fs] fstat async returns correct info", async () => {
  await withTempFileAsync("test data", async (path) => {
    const fd = fs.openSync(path, "r");
    try {
      const stat = await new Promise<fs.Stats>((resolve, reject) => {
        fs.fstat(fd, (err, stat) => {
          if (err) return reject(err);
          resolve(stat);
        });
      });
      assertEquals(stat.isFile(), true);
      assertEquals(stat.size, 9);
    } finally {
      fs.closeSync(fd);
    }
  });
});

// -- ftruncate tests --

Deno.test("[node/fs] ftruncateSync truncates file to given length", () => {
  withTempFile("hello world", (path) => {
    const fd = fs.openSync(path, "r+");
    try {
      fs.ftruncateSync(fd, 5);
    } finally {
      fs.closeSync(fd);
    }
    assertEquals(Deno.readTextFileSync(path), "hello");
  });
});

// -- fsync/fdatasync tests --

Deno.test("[node/fs] fsyncSync does not throw for valid fd", () => {
  withTempFile("data", (path) => {
    const fd = fs.openSync(path, "r+");
    try {
      fs.fsyncSync(fd);
      fs.fdatasyncSync(fd);
    } finally {
      fs.closeSync(fd);
    }
  });
});

// -- readFile/writeFile with fd --

Deno.test("[node/fs] readFileSync with fd reads entire file", () => {
  withTempFile("file content here", (path) => {
    const fd = fs.openSync(path, "r");
    try {
      const data = fs.readFileSync(fd);
      assertEquals(Buffer.from(data).toString(), "file content here");
    } finally {
      fs.closeSync(fd);
    }
  });
});

// -- Multiple fd lifecycle --

Deno.test("[node/fs] multiple fds are independent", () => {
  withTempFile("abcdef", (path) => {
    const fd1 = fs.openSync(path, "r");
    const fd2 = fs.openSync(path, "r");
    try {
      // Read from fd1
      const buf1 = Buffer.alloc(3);
      fs.readSync(fd1, buf1, 0, 3, null);
      assertEquals(buf1.toString(), "abc");

      // fd2 cursor is independent - should read from start
      const buf2 = Buffer.alloc(3);
      fs.readSync(fd2, buf2, 0, 3, null);
      assertEquals(buf2.toString(), "abc");

      // fd1 cursor was advanced to 3
      const buf3 = Buffer.alloc(3);
      fs.readSync(fd1, buf3, 0, 3, null);
      assertEquals(buf3.toString(), "def");
    } finally {
      fs.closeSync(fd1);
      fs.closeSync(fd2);
    }
  });
});

// -- Async open/read/close lifecycle --

Deno.test("[node/fs] async open + read + close lifecycle", async () => {
  await withTempFileAsync("async test data", async (path) => {
    const fd = await new Promise<number>((resolve, reject) => {
      fs.open(path, "r", (err, fd) => {
        if (err) return reject(err);
        resolve(fd);
      });
    });

    const buf = Buffer.alloc(10);
    await new Promise<void>((resolve, reject) => {
      fs.read(fd, buf, 0, 10, 0, (err, nread) => {
        if (err) return reject(err);
        assertEquals(nread, 10);
        assertEquals(buf.toString(), "async test");
        resolve();
      });
    });

    await new Promise<void>((resolve, reject) => {
      fs.close(fd, (err) => {
        if (err) return reject(err);
        resolve();
      });
    });
  });
});

// -- writev/readv with position --

Deno.test("[node/fs] writevSync with position does not move cursor", () => {
  withTempFile("aaaaaaaaaa", (path) => {
    const fd = fs.openSync(path, "r+");
    try {
      const bufs = [Buffer.from("X"), Buffer.from("Y")];
      fs.writevSync(fd, bufs, 4);

      // Cursor should be at 0 still
      fs.writeSync(fd, Buffer.from("ZZ"), 0, 2, null);
    } finally {
      fs.closeSync(fd);
    }
    assertEquals(Deno.readTextFileSync(path), "ZZaaXYaaaa");
  });
});
