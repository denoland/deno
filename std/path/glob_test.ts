import { assert, assertEquals } from "../testing/asserts.ts";
import { testWalk, touch, walkArray } from "../fs/walk_test.ts";
import { globToRegExp, isGlob, joinGlobs, normalizeGlob } from "./glob.ts";
import { SEP, join } from "./mod.ts";

Deno.test({
  name: "glob: glob to regex",
  fn(): void {
    assertEquals(globToRegExp("unicorn.*") instanceof RegExp, true);
    assertEquals(globToRegExp("unicorn.*").test("poney.ts"), false);
    assertEquals(globToRegExp("unicorn.*").test("unicorn.py"), true);
    assertEquals(globToRegExp("*.ts").test("poney.ts"), true);
    assertEquals(globToRegExp("*.ts").test("unicorn.js"), false);
    assertEquals(
      globToRegExp(join("unicorn", "**", "cathedral.ts")).test(
        join("unicorn", "in", "the", "cathedral.ts")
      ),
      true
    );
    assertEquals(
      globToRegExp(join("unicorn", "**", "cathedral.ts")).test(
        join("unicorn", "in", "the", "kitchen.ts")
      ),
      false
    );
    assertEquals(
      globToRegExp(join("unicorn", "**", "bathroom.*")).test(
        join("unicorn", "sleeping", "in", "bathroom.py")
      ),
      true
    );
    assertEquals(
      globToRegExp(join("unicorn", "!(sleeping)", "bathroom.ts"), {
        extended: true,
      }).test(join("unicorn", "flying", "bathroom.ts")),
      true
    );
    assertEquals(
      globToRegExp(join("unicorn", "(!sleeping)", "bathroom.ts"), {
        extended: true,
      }).test(join("unicorn", "sleeping", "bathroom.ts")),
      false
    );
  },
});

testWalk(
  async (d: string): Promise<void> => {
    await Deno.mkdir(d + "/a");
    await Deno.mkdir(d + "/b");
    await touch(d + "/a/x.ts");
    await touch(d + "/b/z.ts");
    await touch(d + "/b/z.js");
  },
  async function globInWalkWildcard(): Promise<void> {
    const arr = await walkArray(".", {
      match: [globToRegExp(join("*", "*.ts"))],
    });
    assertEquals(arr.length, 2);
    assertEquals(arr[0], "a/x.ts");
    assertEquals(arr[1], "b/z.ts");
  }
);

testWalk(
  async (d: string): Promise<void> => {
    await Deno.mkdir(d + "/a");
    await Deno.mkdir(d + "/a/yo");
    await touch(d + "/a/yo/x.ts");
  },
  async function globInWalkFolderWildcard(): Promise<void> {
    const arr = await walkArray(".", {
      match: [
        globToRegExp(join("a", "**", "*.ts"), {
          flags: "g",
          globstar: true,
        }),
      ],
    });
    assertEquals(arr.length, 1);
    assertEquals(arr[0], "a/yo/x.ts");
  }
);

testWalk(
  async (d: string): Promise<void> => {
    await Deno.mkdir(d + "/a");
    await Deno.mkdir(d + "/a/unicorn");
    await Deno.mkdir(d + "/a/deno");
    await Deno.mkdir(d + "/a/raptor");
    await touch(d + "/a/raptor/x.ts");
    await touch(d + "/a/deno/x.ts");
    await touch(d + "/a/unicorn/x.ts");
  },
  async function globInWalkFolderExtended(): Promise<void> {
    const arr = await walkArray(".", {
      match: [
        globToRegExp(join("a", "+(raptor|deno)", "*.ts"), {
          flags: "g",
          extended: true,
        }),
      ],
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
      match: [globToRegExp("x.*", { flags: "g", globstar: true })],
    });
    assertEquals(arr.length, 2);
    assertEquals(arr[0], "x.js");
    assertEquals(arr[1], "x.ts");
  }
);

Deno.test({
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
  },
});

Deno.test("normalizeGlobGlobstar", function (): void {
  assertEquals(normalizeGlob(`**${SEP}..`, { globstar: true }), `**${SEP}..`);
});

Deno.test("joinGlobsGlobstar", function (): void {
  assertEquals(joinGlobs(["**", ".."], { globstar: true }), `**${SEP}..`);
});
