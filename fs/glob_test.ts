const { cwd, mkdir } = Deno;
import { test, runIfMain } from "../testing/mod.ts";
import { assert, assertEquals } from "../testing/asserts.ts";
import { isWindows } from "./path/constants.ts";
import {
  ExpandGlobOptions,
  expandGlob,
  glob,
  isGlob,
  expandGlobSync
} from "./glob.ts";
import { join, normalize, relative } from "./path.ts";
import { WalkInfo } from "./walk.ts";
import { testWalk } from "./walk_test.ts";
import { touch, walkArray } from "./walk_test.ts";

test({
  name: "glob: glob to regex",
  fn(): void {
    assertEquals(glob("unicorn.*") instanceof RegExp, true);
    assertEquals(glob("unicorn.*").test("poney.ts"), false);
    assertEquals(glob("unicorn.*").test("unicorn.py"), true);
    assertEquals(glob("*.ts").test("poney.ts"), true);
    assertEquals(glob("*.ts").test("unicorn.js"), false);
    assertEquals(
      glob(join("unicorn", "**", "cathedral.ts")).test(
        join("unicorn", "in", "the", "cathedral.ts")
      ),
      true
    );
    assertEquals(
      glob(join("unicorn", "**", "cathedral.ts")).test(
        join("unicorn", "in", "the", "kitchen.ts")
      ),
      false
    );
    assertEquals(
      glob(join("unicorn", "**", "bathroom.*")).test(
        join("unicorn", "sleeping", "in", "bathroom.py")
      ),
      true
    );
    assertEquals(
      glob(join("unicorn", "!(sleeping)", "bathroom.ts"), {
        extended: true
      }).test(join("unicorn", "flying", "bathroom.ts")),
      true
    );
    assertEquals(
      glob(join("unicorn", "(!sleeping)", "bathroom.ts"), {
        extended: true
      }).test(join("unicorn", "sleeping", "bathroom.ts")),
      false
    );
  }
});

testWalk(
  async (d: string): Promise<void> => {
    await mkdir(d + "/a");
    await touch(d + "/a/x.ts");
  },
  async function globInWalk(): Promise<void> {
    const arr = await walkArray(".", { match: [glob("*.ts")] });
    assertEquals(arr.length, 1);
    assertEquals(arr[0], "a/x.ts");
  }
);

testWalk(
  async (d: string): Promise<void> => {
    await mkdir(d + "/a");
    await mkdir(d + "/b");
    await touch(d + "/a/x.ts");
    await touch(d + "/b/z.ts");
    await touch(d + "/b/z.js");
  },
  async function globInWalkWildcardFiles(): Promise<void> {
    const arr = await walkArray(".", { match: [glob("*.ts")] });
    assertEquals(arr.length, 2);
    assertEquals(arr[0], "a/x.ts");
    assertEquals(arr[1], "b/z.ts");
  }
);

testWalk(
  async (d: string): Promise<void> => {
    await mkdir(d + "/a");
    await mkdir(d + "/a/yo");
    await touch(d + "/a/yo/x.ts");
  },
  async function globInWalkFolderWildcard(): Promise<void> {
    const arr = await walkArray(".", {
      match: [
        glob(join("a", "**", "*.ts"), {
          flags: "g",
          globstar: true
        })
      ]
    });
    assertEquals(arr.length, 1);
    assertEquals(arr[0], "a/yo/x.ts");
  }
);

testWalk(
  async (d: string): Promise<void> => {
    await mkdir(d + "/a");
    await mkdir(d + "/a/unicorn");
    await mkdir(d + "/a/deno");
    await mkdir(d + "/a/raptor");
    await touch(d + "/a/raptor/x.ts");
    await touch(d + "/a/deno/x.ts");
    await touch(d + "/a/unicorn/x.ts");
  },
  async function globInWalkFolderExtended(): Promise<void> {
    const arr = await walkArray(".", {
      match: [
        glob(join("a", "+(raptor|deno)", "*.ts"), {
          flags: "g",
          extended: true
        })
      ]
    });
    assertEquals(arr.length, 2);
    assertEquals(arr[0], "a/deno/x.ts");
    assertEquals(arr[1], "a/raptor/x.ts");
  }
);

testWalk(
  async (d: string): Promise<void> => {
    await touch(d + "/x.ts");
    await touch(d + "/x.js");
    await touch(d + "/b.js");
  },
  async function globInWalkWildcardExtension(): Promise<void> {
    const arr = await walkArray(".", {
      match: [glob("x.*", { flags: "g", globstar: true })]
    });
    assertEquals(arr.length, 2);
    assertEquals(arr[0], "x.js");
    assertEquals(arr[1], "x.ts");
  }
);

test({
  name: "isGlob: pattern to test",
  fn(): void {
    // should be true if valid glob pattern
    assert(isGlob("!foo.js"));
    assert(isGlob("*.js"));
    assert(isGlob("!*.js"));
    assert(isGlob("!foo"));
    assert(isGlob("!foo.js"));
    assert(isGlob("**/abc.js"));
    assert(isGlob("abc/*.js"));
    assert(isGlob("@.(?:abc)"));
    assert(isGlob("@.(?!abc)"));

    // should be false if invalid glob pattern
    assert(!isGlob(""));
    assert(!isGlob("~/abc"));
    assert(!isGlob("~/abc"));
    assert(!isGlob("~/(abc)"));
    assert(!isGlob("+~(abc)"));
    assert(!isGlob("."));
    assert(!isGlob("@.(abc)"));
    assert(!isGlob("aa"));
    assert(!isGlob("who?"));
    assert(!isGlob("why!?"));
    assert(!isGlob("where???"));
    assert(!isGlob("abc!/def/!ghi.js"));
    assert(!isGlob("abc.js"));
    assert(!isGlob("abc/def/!ghi.js"));
    assert(!isGlob("abc/def/ghi.js"));

    // Should be true if path has regex capture group
    assert(isGlob("abc/(?!foo).js"));
    assert(isGlob("abc/(?:foo).js"));
    assert(isGlob("abc/(?=foo).js"));
    assert(isGlob("abc/(a|b).js"));
    assert(isGlob("abc/(a|b|c).js"));
    assert(isGlob("abc/(foo bar)/*.js"));

    // Should be false if the path has parens but is not a valid capture group
    assert(!isGlob("abc/(?foo).js"));
    assert(!isGlob("abc/(a b c).js"));
    assert(!isGlob("abc/(ab).js"));
    assert(!isGlob("abc/(abc).js"));
    assert(!isGlob("abc/(foo bar).js"));

    // should be false if the capture group is imbalanced
    assert(!isGlob("abc/(?ab.js"));
    assert(!isGlob("abc/(ab.js"));
    assert(!isGlob("abc/(a|b.js"));
    assert(!isGlob("abc/(a|b|c.js"));

    // should be true if the path has a regex character class
    assert(isGlob("abc/[abc].js"));
    assert(isGlob("abc/[^abc].js"));
    assert(isGlob("abc/[1-3].js"));

    // should be false if the character class is not balanced
    assert(!isGlob("abc/[abc.js"));
    assert(!isGlob("abc/[^abc.js"));
    assert(!isGlob("abc/[1-3.js"));

    // should be false if the character class is escaped
    assert(!isGlob("abc/\\[abc].js"));
    assert(!isGlob("abc/\\[^abc].js"));
    assert(!isGlob("abc/\\[1-3].js"));

    // should be true if the path has brace characters
    assert(isGlob("abc/{a,b}.js"));
    assert(isGlob("abc/{a..z}.js"));
    assert(isGlob("abc/{a..z..2}.js"));

    // should be false if (basic) braces are not balanced
    assert(!isGlob("abc/\\{a,b}.js"));
    assert(!isGlob("abc/\\{a..z}.js"));
    assert(!isGlob("abc/\\{a..z..2}.js"));

    // should be true if the path has regex characters
    assert(isGlob("!&(abc)"));
    assert(isGlob("!*.js"));
    assert(isGlob("!foo"));
    assert(isGlob("!foo.js"));
    assert(isGlob("**/abc.js"));
    assert(isGlob("*.js"));
    assert(isGlob("*z(abc)"));
    assert(isGlob("[1-10].js"));
    assert(isGlob("[^abc].js"));
    assert(isGlob("[a-j]*[^c]b/c"));
    assert(isGlob("[abc].js"));
    assert(isGlob("a/b/c/[a-z].js"));
    assert(isGlob("abc/(aaa|bbb).js"));
    assert(isGlob("abc/*.js"));
    assert(isGlob("abc/{a,b}.js"));
    assert(isGlob("abc/{a..z..2}.js"));
    assert(isGlob("abc/{a..z}.js"));

    assert(!isGlob("$(abc)"));
    assert(!isGlob("&(abc)"));
    assert(!isGlob("Who?.js"));
    assert(!isGlob("? (abc)"));
    assert(!isGlob("?.js"));
    assert(!isGlob("abc/?.js"));

    // should be false if regex characters are escaped
    assert(!isGlob("\\?.js"));
    assert(!isGlob("\\[1-10\\].js"));
    assert(!isGlob("\\[^abc\\].js"));
    assert(!isGlob("\\[a-j\\]\\*\\[^c\\]b/c"));
    assert(!isGlob("\\[abc\\].js"));
    assert(!isGlob("\\a/b/c/\\[a-z\\].js"));
    assert(!isGlob("abc/\\(aaa|bbb).js"));
    assert(!isGlob("abc/\\?.js"));
  }
});

async function expandGlobArray(
  globString: string,
  options: ExpandGlobOptions
): Promise<string[]> {
  const infos: WalkInfo[] = [];
  for await (const info of expandGlob(globString, options)) {
    infos.push(info);
  }
  infos.sort();
  const infosSync = [...expandGlobSync(globString, options)];
  infosSync.sort();
  assertEquals(infos, infosSync);
  const root = normalize(options.root || cwd());
  const paths = infos.map(({ filename }): string => filename);
  for (const path of paths) {
    assert(path.startsWith(root));
  }
  const relativePaths = paths.map((path: string): string =>
    relative(root, path)
  );
  relativePaths.sort();
  return relativePaths;
}

function urlToFilePath(url: URL): string {
  // Since `new URL('file:///C:/a').pathname` is `/C:/a`, remove leading slash.
  return url.pathname.slice(url.protocol == "file:" && isWindows ? 1 : 0);
}

const EG_OPTIONS = {
  root: urlToFilePath(new URL(join("testdata", "glob"), import.meta.url)),
  includeDirs: true,
  extended: false,
  globstar: false,
  strict: false,
  filepath: false,
  flags: ""
};

test(async function expandGlobExt(): Promise<void> {
  const options = { ...EG_OPTIONS, extended: true };
  assertEquals(await expandGlobArray("abc?(def|ghi)", options), [
    "abc",
    "abcdef"
  ]);
  assertEquals(await expandGlobArray("abc*(def|ghi)", options), [
    "abc",
    "abcdef",
    "abcdefghi"
  ]);
  assertEquals(await expandGlobArray("abc+(def|ghi)", options), [
    "abcdef",
    "abcdefghi"
  ]);
  assertEquals(await expandGlobArray("abc@(def|ghi)", options), ["abcdef"]);
  assertEquals(await expandGlobArray("abc{def,ghi}", options), ["abcdef"]);
  assertEquals(await expandGlobArray("abc!(def|ghi)", options), ["abc"]);
});

test(async function expandGlobGlobstar(): Promise<void> {
  const options = { ...EG_OPTIONS, globstar: true };
  assertEquals(await expandGlobArray(join("**", "abc"), options), [
    "abc",
    join("subdir", "abc")
  ]);
});

test(async function expandGlobIncludeDirs(): Promise<void> {
  const options = { ...EG_OPTIONS, includeDirs: false };
  assertEquals(await expandGlobArray("subdir", options), []);
});

runIfMain(import.meta);
