const { mkdir, open } = Deno;
import { FileInfo } from "deno";
import { test } from "../testing/mod.ts";
import { assertEq } from "../testing/asserts.ts";
import { glob } from "./glob.ts";
import { join } from "./path.ts";
import { testWalk } from "./walk_test.ts";
import { walk, walkSync, WalkOptions } from "./walk.ts";

async function touch(path: string): Promise<void> {
  await open(path, "w");
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

test({
  name: "glob: glob to regex",
  fn() {
    assertEq(glob("unicorn.*") instanceof RegExp, true);
    assertEq(glob("unicorn.*").test("poney.ts"), false);
    assertEq(glob("unicorn.*").test("unicorn.py"), true);
    assertEq(glob("*.ts").test("poney.ts"), true);
    assertEq(glob("*.ts").test("unicorn.js"), false);
    assertEq(
      glob(join("unicorn", "**", "cathedral.ts")).test(
        join("unicorn", "in", "the", "cathedral.ts")
      ),
      true
    );
    assertEq(
      glob(join("unicorn", "**", "cathedral.ts")).test(
        join("unicorn", "in", "the", "kitchen.ts")
      ),
      false
    );
    assertEq(
      glob(join("unicorn", "**", "bathroom.*")).test(
        join("unicorn", "sleeping", "in", "bathroom.py")
      ),
      true
    );
    assertEq(
      glob(join("unicorn", "!(sleeping)", "bathroom.ts"), {
        extended: true
      }).test(join("unicorn", "flying", "bathroom.ts")),
      true
    );
    assertEq(
      glob(join("unicorn", "(!sleeping)", "bathroom.ts"), {
        extended: true
      }).test(join("unicorn", "sleeping", "bathroom.ts")),
      false
    );
  }
});

testWalk(
  async (d: string) => {
    await mkdir(d + "/a");
    await touch(d + "/a/x.ts");
  },
  async function globInWalk() {
    const arr = await walkArray(".", { match: [glob("*.ts")] });
    assertEq(arr.length, 1);
    assertEq(arr[0], "./a/x.ts");
  }
);

testWalk(
  async (d: string) => {
    await mkdir(d + "/a");
    await mkdir(d + "/b");
    await touch(d + "/a/x.ts");
    await touch(d + "/b/z.ts");
    await touch(d + "/b/z.js");
  },
  async function globInWalkWildcardFiles() {
    const arr = await walkArray(".", { match: [glob("*.ts")] });
    assertEq(arr.length, 2);
    assertEq(arr[0], "./a/x.ts");
    assertEq(arr[1], "./b/z.ts");
  }
);

testWalk(
  async (d: string) => {
    await mkdir(d + "/a");
    await mkdir(d + "/a/yo");
    await touch(d + "/a/yo/x.ts");
  },
  async function globInWalkFolderWildcard() {
    const arr = await walkArray(".", {
      match: [
        glob(join("a", "**", "*.ts"), {
          flags: "g",
          globstar: true
        })
      ]
    });
    assertEq(arr.length, 1);
    assertEq(arr[0], "./a/yo/x.ts");
  }
);

testWalk(
  async (d: string) => {
    await mkdir(d + "/a");
    await mkdir(d + "/a/unicorn");
    await mkdir(d + "/a/deno");
    await mkdir(d + "/a/raptor");
    await touch(d + "/a/raptor/x.ts");
    await touch(d + "/a/deno/x.ts");
    await touch(d + "/a/unicorn/x.ts");
  },
  async function globInWalkFolderExtended() {
    const arr = await walkArray(".", {
      match: [
        glob(join("a", "+(raptor|deno)", "*.ts"), {
          flags: "g",
          extended: true
        })
      ]
    });
    assertEq(arr.length, 2);
    assertEq(arr[0], "./a/deno/x.ts");
    assertEq(arr[1], "./a/raptor/x.ts");
  }
);

testWalk(
  async (d: string) => {
    await touch(d + "/x.ts");
    await touch(d + "/x.js");
    await touch(d + "/b.js");
  },
  async function globInWalkWildcardExtension() {
    const arr = await walkArray(".", {
      match: [glob("x.*", { flags: "g", globstar: true })]
    });
    console.log(arr);
    assertEq(arr.length, 2);
    assertEq(arr[0], "./x.js");
    assertEq(arr[1], "./x.ts");
  }
);
