import { walk, walkSync, WalkOptions, WalkEntry } from "./walk.ts";
import { assert, assertEquals, assertThrowsAsync } from "../testing/asserts.ts";

export function testWalk(
  setup: (arg0: string) => void | Promise<void>,
  t: () => void | Promise<void>,
  ignore = false
): void {
  const name = t.name;
  async function fn(): Promise<void> {
    const origCwd = Deno.cwd();
    const d = await Deno.makeTempDir();
    Deno.chdir(d);
    try {
      await setup(d);
      await t();
    } finally {
      Deno.chdir(origCwd);
      await Deno.remove(d, { recursive: true });
    }
  }
  Deno.test({ ignore, name: `[walk] ${name}`, fn });
}

function normalize({ path }: WalkEntry): string {
  return path.replace(/\\/g, "/");
}

export async function walkArray(
  root: string,
  options: WalkOptions = {}
): Promise<string[]> {
  const arr: string[] = [];
  for await (const w of walk(root, { ...options })) {
    arr.push(normalize(w));
  }
  arr.sort(); // TODO(ry) Remove sort. The order should be deterministic.
  const arrSync = Array.from(walkSync(root, options), normalize);
  arrSync.sort(); // TODO(ry) Remove sort. The order should be deterministic.
  assertEquals(arr, arrSync);
  return arr;
}

export async function touch(path: string): Promise<void> {
  const f = await Deno.create(path);
  f.close();
}

function assertReady(expectedLength: number): void {
  const arr = Array.from(walkSync("."), normalize);

  assertEquals(arr.length, expectedLength);
}

testWalk(
  async (d: string): Promise<void> => {
    await Deno.mkdir(d + "/empty");
  },
  async function emptyDir(): Promise<void> {
    const arr = await walkArray(".");
    assertEquals(arr, [".", "empty"]);
  }
);

testWalk(
  async (d: string): Promise<void> => {
    await touch(d + "/x");
  },
  async function singleFile(): Promise<void> {
    const arr = await walkArray(".");
    assertEquals(arr, [".", "x"]);
  }
);

testWalk(
  async (d: string): Promise<void> => {
    await touch(d + "/x");
  },
  async function iteratable(): Promise<void> {
    let count = 0;
    for (const _ of walkSync(".")) {
      count += 1;
    }
    assertEquals(count, 2);
    for await (const _ of walk(".")) {
      count += 1;
    }
    assertEquals(count, 4);
  }
);

testWalk(
  async (d: string): Promise<void> => {
    await Deno.mkdir(d + "/a");
    await touch(d + "/a/x");
  },
  async function nestedSingleFile(): Promise<void> {
    const arr = await walkArray(".");
    assertEquals(arr, [".", "a", "a/x"]);
  }
);

testWalk(
  async (d: string): Promise<void> => {
    await Deno.mkdir(d + "/a/b/c/d", { recursive: true });
    await touch(d + "/a/b/c/d/x");
  },
  async function depth(): Promise<void> {
    assertReady(6);
    const arr3 = await walkArray(".", { maxDepth: 3 });
    assertEquals(arr3, [".", "a", "a/b", "a/b/c"]);
    const arr5 = await walkArray(".", { maxDepth: 5 });
    assertEquals(arr5, [".", "a", "a/b", "a/b/c", "a/b/c/d", "a/b/c/d/x"]);
  }
);

testWalk(
  async (d: string): Promise<void> => {
    await touch(d + "/a");
    await Deno.mkdir(d + "/b");
    await touch(d + "/b/c");
  },
  async function includeDirs(): Promise<void> {
    assertReady(4);
    const arr = await walkArray(".", { includeDirs: false });
    assertEquals(arr, ["a", "b/c"]);
  }
);

testWalk(
  async (d: string): Promise<void> => {
    await touch(d + "/a");
    await Deno.mkdir(d + "/b");
    await touch(d + "/b/c");
  },
  async function includeFiles(): Promise<void> {
    assertReady(4);
    const arr = await walkArray(".", { includeFiles: false });
    assertEquals(arr, [".", "b"]);
  }
);

testWalk(
  async (d: string): Promise<void> => {
    await touch(d + "/x.ts");
    await touch(d + "/y.rs");
  },
  async function ext(): Promise<void> {
    assertReady(3);
    const arr = await walkArray(".", { exts: [".ts"] });
    assertEquals(arr, ["x.ts"]);
  }
);

testWalk(
  async (d: string): Promise<void> => {
    await touch(d + "/x.ts");
    await touch(d + "/y.rs");
    await touch(d + "/z.py");
  },
  async function extAny(): Promise<void> {
    assertReady(4);
    const arr = await walkArray(".", { exts: [".rs", ".ts"] });
    assertEquals(arr, ["x.ts", "y.rs"]);
  }
);

testWalk(
  async (d: string): Promise<void> => {
    await touch(d + "/x");
    await touch(d + "/y");
  },
  async function match(): Promise<void> {
    assertReady(3);
    const arr = await walkArray(".", { match: [/x/] });
    assertEquals(arr, ["x"]);
  }
);

testWalk(
  async (d: string): Promise<void> => {
    await touch(d + "/x");
    await touch(d + "/y");
    await touch(d + "/z");
  },
  async function matchAny(): Promise<void> {
    assertReady(4);
    const arr = await walkArray(".", { match: [/x/, /y/] });
    assertEquals(arr, ["x", "y"]);
  }
);

testWalk(
  async (d: string): Promise<void> => {
    await touch(d + "/x");
    await touch(d + "/y");
  },
  async function skip(): Promise<void> {
    assertReady(3);
    const arr = await walkArray(".", { skip: [/x/] });
    assertEquals(arr, [".", "y"]);
  }
);

testWalk(
  async (d: string): Promise<void> => {
    await touch(d + "/x");
    await touch(d + "/y");
    await touch(d + "/z");
  },
  async function skipAny(): Promise<void> {
    assertReady(4);
    const arr = await walkArray(".", { skip: [/x/, /y/] });
    assertEquals(arr, [".", "z"]);
  }
);

testWalk(
  async (d: string): Promise<void> => {
    await Deno.mkdir(d + "/a");
    await Deno.mkdir(d + "/b");
    await touch(d + "/a/x");
    await touch(d + "/a/y");
    await touch(d + "/b/z");
  },
  async function subDir(): Promise<void> {
    assertReady(6);
    const arr = await walkArray("b");
    assertEquals(arr, ["b", "b/z"]);
  }
);

testWalk(
  async (_d: string): Promise<void> => {},
  async function nonexistentRoot(): Promise<void> {
    await assertThrowsAsync(async () => {
      await walkArray("nonexistent");
    }, Deno.errors.NotFound);
  }
);

// TODO(ry) Re-enable followSymlinks
testWalk(
  async (d: string): Promise<void> => {
    await Deno.mkdir(d + "/a");
    await Deno.mkdir(d + "/b");
    await touch(d + "/a/x");
    await touch(d + "/a/y");
    await touch(d + "/b/z");
    try {
      await Deno.symlink(d + "/b", d + "/a/bb");
    } catch (err) {
      assert(Deno.build.os == "windows");
      assertEquals(err.message, "Not implemented");
    }
  },
  async function symlink(): Promise<void> {
    // symlink is not yet implemented on Windows.
    if (Deno.build.os == "windows") {
      return;
    }

    assertReady(6);
    const files = await walkArray("a");
    assertEquals(files.length, 2);
    assert(!files.includes("a/bb/z"));

    const arr = await walkArray("a", { followSymlinks: true });
    assertEquals(arr.length, 3);
    assert(arr.some((f): boolean => f.endsWith("/b/z")));
  },
  true
);
