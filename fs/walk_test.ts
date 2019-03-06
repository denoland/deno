const {
  cwd,
  chdir,
  makeTempDir,
  mkdir,
  open,
  platform,
  remove,
  symlink
} = Deno;
import { FileInfo } from "deno";
import { walk, walkSync, WalkOptions } from "./walk.ts";
import { test, TestFunction } from "../testing/mod.ts";
import { assert, assertEq } from "../testing/asserts.ts";

const isWindows = platform.os === "win";

export async function testWalk(
  setup: (string) => void | Promise<void>,
  t: TestFunction
): Promise<void> {
  const name = t.name;
  async function fn() {
    const orig_cwd = cwd();
    const d = await makeTempDir();
    chdir(d);
    try {
      await setup(d);
      await t();
    } finally {
      chdir(orig_cwd);
      remove(d, { recursive: true });
    }
  }
  test({ name, fn });
}

async function walkArray(
  dirname: string = ".",
  options: WalkOptions = {}
): Promise<Array<string>> {
  const arr: string[] = [];
  for await (const f of walk(dirname, { ...options })) {
    arr.push(f.path.replace(/\\/g, "/"));
  }
  arr.sort();
  const arr_sync = Array.from(walkSync(dirname, options), (f: FileInfo) =>
    f.path.replace(/\\/g, "/")
  ).sort();
  assertEq(arr, arr_sync);
  return arr;
}

async function touch(path: string): Promise<void> {
  await open(path, "w");
}
function assertReady(expectedLength: number) {
  const arr = Array.from(walkSync(), (f: FileInfo) => f.path);
  assertEq(arr.length, expectedLength);
}

testWalk(
  async (d: string) => {
    await mkdir(d + "/empty");
  },
  async function emptyDir() {
    const arr = await walkArray();
    assertEq(arr.length, 0);
  }
);

testWalk(
  async (d: string) => {
    await touch(d + "/x");
  },
  async function singleFile() {
    const arr = await walkArray();
    assertEq(arr.length, 1);
    assertEq(arr[0], "./x");
  }
);

testWalk(
  async (d: string) => {
    await touch(d + "/x");
  },
  async function iteratable() {
    let count = 0;
    for (const f of walkSync()) {
      count += 1;
    }
    assertEq(count, 1);
    for await (const f of walk()) {
      count += 1;
    }
    assertEq(count, 2);
  }
);

testWalk(
  async (d: string) => {
    await mkdir(d + "/a");
    await touch(d + "/a/x");
  },
  async function nestedSingleFile() {
    const arr = await walkArray();
    assertEq(arr.length, 1);
    assertEq(arr[0], "./a/x");
  }
);

testWalk(
  async (d: string) => {
    await mkdir(d + "/a/b/c/d", true);
    await touch(d + "/a/b/c/d/x");
  },
  async function depth() {
    assertReady(1);
    const arr_3 = await walkArray(".", { maxDepth: 3 });
    assertEq(arr_3.length, 0);
    const arr_5 = await walkArray(".", { maxDepth: 5 });
    assertEq(arr_5.length, 1);
    assertEq(arr_5[0], "./a/b/c/d/x");
  }
);

testWalk(
  async (d: string) => {
    await touch(d + "/x.ts");
    await touch(d + "/y.rs");
  },
  async function ext() {
    assertReady(2);
    const arr = await walkArray(".", { exts: [".ts"] });
    assertEq(arr.length, 1);
    assertEq(arr[0], "./x.ts");
  }
);

testWalk(
  async (d: string) => {
    await touch(d + "/x.ts");
    await touch(d + "/y.rs");
    await touch(d + "/z.py");
  },
  async function extAny() {
    assertReady(3);
    const arr = await walkArray(".", { exts: [".rs", ".ts"] });
    assertEq(arr.length, 2);
    assertEq(arr[0], "./x.ts");
    assertEq(arr[1], "./y.rs");
  }
);

testWalk(
  async (d: string) => {
    await touch(d + "/x");
    await touch(d + "/y");
  },
  async function match() {
    assertReady(2);
    const arr = await walkArray(".", { match: [/x/] });
    assertEq(arr.length, 1);
    assertEq(arr[0], "./x");
  }
);

testWalk(
  async (d: string) => {
    await touch(d + "/x");
    await touch(d + "/y");
    await touch(d + "/z");
  },
  async function matchAny() {
    assertReady(3);
    const arr = await walkArray(".", { match: [/x/, /y/] });
    assertEq(arr.length, 2);
    assertEq(arr[0], "./x");
    assertEq(arr[1], "./y");
  }
);

testWalk(
  async (d: string) => {
    await touch(d + "/x");
    await touch(d + "/y");
  },
  async function skip() {
    assertReady(2);
    const arr = await walkArray(".", { skip: [/x/] });
    assertEq(arr.length, 1);
    assertEq(arr[0], "./y");
  }
);

testWalk(
  async (d: string) => {
    await touch(d + "/x");
    await touch(d + "/y");
    await touch(d + "/z");
  },
  async function skipAny() {
    assertReady(3);
    const arr = await walkArray(".", { skip: [/x/, /y/] });
    assertEq(arr.length, 1);
    assertEq(arr[0], "./z");
  }
);

testWalk(
  async (d: string) => {
    await mkdir(d + "/a");
    await mkdir(d + "/b");
    await touch(d + "/a/x");
    await touch(d + "/a/y");
    await touch(d + "/b/z");
  },
  async function subDir() {
    assertReady(3);
    const arr = await walkArray("b");
    assertEq(arr.length, 1);
    assertEq(arr[0], "b/z");
  }
);

testWalk(async (d: string) => {}, async function onError() {
  assertReady(0);
  const ignored = await walkArray("missing");
  assertEq(ignored.length, 0);
  let errors = 0;
  const arr = await walkArray("missing", { onError: e => (errors += 1) });
  // It's 2 since walkArray iterates over both sync and async.
  assertEq(errors, 2);
});

testWalk(
  async (d: string) => {
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
  async function symlink() {
    // symlink is not yet implemented on Windows.
    if (isWindows) {
      return;
    }

    assertReady(3);
    const files = await walkArray("a");
    assertEq(files.length, 2);
    assert(!files.includes("a/bb/z"));

    const arr = await walkArray("a", { followSymlinks: true });
    assertEq(arr.length, 3);
    assert(arr.some(f => f.endsWith("/b/z")));
  }
);
