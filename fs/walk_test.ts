const { cwd, chdir, makeTempDir, mkdir, open, remove } = Deno;
type FileInfo = Deno.FileInfo;
import { walk, walkSync, WalkOptions, WalkInfo } from "./walk.ts";
import { test, TestFunction, runIfMain } from "../testing/mod.ts";
import { assertEquals } from "../testing/asserts.ts";

export async function testWalk(
  setup: (string) => void | Promise<void>,
  t: TestFunction
): Promise<void> {
  const name = t.name;
  async function fn(): Promise<void> {
    const origCwd = cwd();
    const d = await makeTempDir();
    chdir(d);
    try {
      await setup(d);
      await t();
    } finally {
      chdir(origCwd);
      remove(d, { recursive: true });
    }
  }
  test({ name, fn });
}

function normalize({ filename }: WalkInfo): string {
  return filename.replace(/\\/g, "/");
}

export async function walkArray(
  root: string = ".",
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
  await open(path, "w");
}

function assertReady(expectedLength: number): void {
  const arr = Array.from(walkSync(), normalize);

  assertEquals(arr.length, expectedLength);
}

testWalk(
  async (d: string): Promise<void> => {
    await mkdir(d + "/empty");
  },
  async function emptyDir(): Promise<void> {
    const arr = await walkArray();
    assertEquals(arr.length, 0);
  }
);

testWalk(
  async (d: string): Promise<void> => {
    await touch(d + "/x");
  },
  async function singleFile(): Promise<void> {
    const arr = await walkArray();
    assertEquals(arr.length, 1);
    assertEquals(arr[0], "x");
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
    assertEquals(count, 1);
    for await (const _ of walk(".")) {
      count += 1;
    }
    assertEquals(count, 2);
  }
);

testWalk(
  async (d: string): Promise<void> => {
    await mkdir(d + "/a");
    await touch(d + "/a/x");
  },
  async function nestedSingleFile(): Promise<void> {
    const arr = await walkArray();
    assertEquals(arr.length, 1);
    assertEquals(arr[0], "a/x");
  }
);

testWalk(
  async (d: string): Promise<void> => {
    await mkdir(d + "/a/b/c/d", true);
    await touch(d + "/a/b/c/d/x");
  },
  async function depth(): Promise<void> {
    assertReady(1);
    const arr3 = await walkArray(".", { maxDepth: 3 });
    assertEquals(arr3.length, 0);
    const arr5 = await walkArray(".", { maxDepth: 5 });
    assertEquals(arr5.length, 1);
    assertEquals(arr5[0], "a/b/c/d/x");
  }
);

testWalk(
  async (d: string): Promise<void> => {
    await touch(d + "/x.ts");
    await touch(d + "/y.rs");
  },
  async function ext(): Promise<void> {
    assertReady(2);
    const arr = await walkArray(".", { exts: [".ts"] });
    assertEquals(arr.length, 1);
    assertEquals(arr[0], "x.ts");
  }
);

testWalk(
  async (d: string): Promise<void> => {
    await touch(d + "/x.ts");
    await touch(d + "/y.rs");
    await touch(d + "/z.py");
  },
  async function extAny(): Promise<void> {
    assertReady(3);
    const arr = await walkArray(".", { exts: [".rs", ".ts"] });
    assertEquals(arr.length, 2);
    assertEquals(arr[0], "x.ts");
    assertEquals(arr[1], "y.rs");
  }
);

testWalk(
  async (d: string): Promise<void> => {
    await touch(d + "/x");
    await touch(d + "/y");
  },
  async function match(): Promise<void> {
    assertReady(2);
    const arr = await walkArray(".", { match: [/x/] });
    assertEquals(arr.length, 1);
    assertEquals(arr[0], "x");
  }
);

testWalk(
  async (d: string): Promise<void> => {
    await touch(d + "/x");
    await touch(d + "/y");
    await touch(d + "/z");
  },
  async function matchAny(): Promise<void> {
    assertReady(3);
    const arr = await walkArray(".", { match: [/x/, /y/] });
    assertEquals(arr.length, 2);
    assertEquals(arr[0], "x");
    assertEquals(arr[1], "y");
  }
);

testWalk(
  async (d: string): Promise<void> => {
    await touch(d + "/x");
    await touch(d + "/y");
  },
  async function skip(): Promise<void> {
    assertReady(2);
    const arr = await walkArray(".", { skip: [/x/] });
    assertEquals(arr.length, 1);
    assertEquals(arr[0], "y");
  }
);

testWalk(
  async (d: string): Promise<void> => {
    await touch(d + "/x");
    await touch(d + "/y");
    await touch(d + "/z");
  },
  async function skipAny(): Promise<void> {
    assertReady(3);
    const arr = await walkArray(".", { skip: [/x/, /y/] });
    assertEquals(arr.length, 1);
    assertEquals(arr[0], "z");
  }
);

testWalk(
  async (d: string): Promise<void> => {
    await mkdir(d + "/a");
    await mkdir(d + "/b");
    await touch(d + "/a/x");
    await touch(d + "/a/y");
    await touch(d + "/b/z");
  },
  async function subDir(): Promise<void> {
    assertReady(3);
    const arr = await walkArray("b");
    assertEquals(arr.length, 1);
    assertEquals(arr[0], "b/z");
  }
);

testWalk(
  async (_d: string): Promise<void> => {},
  async function onError(): Promise<void> {
    assertReady(0);
    const ignored = await walkArray("missing");
    assertEquals(ignored.length, 0);
    let errors = 0;
    await walkArray("missing", { onError: (_e): number => (errors += 1) });
    // It's 2 since walkArray iterates over both sync and async.
    assertEquals(errors, 2);
  }
);

/* TODO(ry) Re-enable followSymlinks
testWalk(
  async (d: string): Promise<void> => {
    await mkdir(d + "/a");
    await mkdir(d + "/b");
    await touch(d + "/a/x");
    await touch(d + "/a/y");
    await touch(d + "/b/z");
    try {
      await symlink(d + "/b", d + "/a/bb");
    } catch (err) {
      assert(isWindows);
      assert(err.message, "Not implemented");
    }
  },
  async function symlink(): Promise<void> {
    // symlink is not yet implemented on Windows.
    if (isWindows) {
      return;
    }

    assertReady(3);
    const files = await walkArray("a");
    assertEquals(files.length, 2);
    assert(!files.includes("a/bb/z"));

    const arr = await walkArray("a", { followSymlinks: true });
    assertEquals(arr.length, 3);
    assert(arr.some((f): boolean => f.endsWith("/b/z")));
  }
);
*/

runIfMain(import.meta);
