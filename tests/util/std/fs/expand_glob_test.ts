// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { assert, assertEquals, assertStringIncludes } from "../assert/mod.ts";
import {
  fromFileUrl,
  join,
  joinGlobs,
  normalize,
  relative,
} from "../path/mod.ts";
import {
  expandGlob,
  ExpandGlobOptions,
  expandGlobSync,
} from "./expand_glob.ts";

async function expandGlobArray(
  globString: string,
  options: ExpandGlobOptions,
  { forceRoot = "" } = {},
): Promise<string[]> {
  const paths = await Array.fromAsync(
    expandGlob(globString, options),
    ({ path }) => path,
  );
  paths.sort();
  const root = normalize(forceRoot || options.root || Deno.cwd());
  for (const path of paths) {
    assert(path.startsWith(root));
  }
  const relativePaths = paths.map(
    (path: string): string => relative(root, path) || ".",
  );
  relativePaths.sort();
  return relativePaths;
}

function expandGlobSyncArray(
  globString: string,
  options: ExpandGlobOptions,
  { forceRoot = "" } = {},
): string[] {
  const pathsSync = [...expandGlobSync(globString, options)].map(
    ({ path }): string => path,
  );
  pathsSync.sort();
  const root = normalize(forceRoot || options.root || Deno.cwd());
  for (const path of pathsSync) {
    assert(path.startsWith(root));
  }
  const relativePaths = pathsSync.map(
    (path: string): string => relative(root, path) || ".",
  );
  relativePaths.sort();
  return relativePaths;
}

const EG_OPTIONS: ExpandGlobOptions = {
  root: fromFileUrl(new URL(join("testdata", "glob"), import.meta.url)),
  includeDirs: true,
  extended: false,
};

Deno.test("expandGlob() with wildcard input returns all test data", async function () {
  const options = EG_OPTIONS;
  assertEquals(await expandGlobArray("*", options), [
    "a[b]c",
    "abc",
    "abcdef",
    "abcdefghi",
    "link",
    "subdir",
  ]);
});

Deno.test("expandGlobSync() with wildcard input returns all test data", function () {
  const options = EG_OPTIONS;
  assertEquals(expandGlobSyncArray("*", options), [
    "a[b]c",
    "abc",
    "abcdef",
    "abcdefghi",
    "link",
    "subdir",
  ]);
});

Deno.test("expandGlob() with */ input returns subdirs", async function () {
  const options = EG_OPTIONS;
  assertEquals(await expandGlobArray("*/", options), [
    "a[b]c",
    "subdir",
  ]);
});

Deno.test("expandGlobSync() with */ input returns subdirs", function () {
  const options = EG_OPTIONS;
  assertEquals(expandGlobSyncArray("*/", options), [
    "a[b]c",
    "subdir",
  ]);
});

Deno.test("expandGlob() with subdir/../* input expands parent", async function () {
  const options = EG_OPTIONS;
  assertEquals(await expandGlobArray("subdir/../*", options), [
    "a[b]c",
    "abc",
    "abcdef",
    "abcdefghi",
    "link",
    "subdir",
  ]);
});

Deno.test("expandGlobSync() with subdir/../* input expands parent", function () {
  const options = EG_OPTIONS;
  assertEquals(expandGlobSyncArray("subdir/../*", options), [
    "a[b]c",
    "abc",
    "abcdef",
    "abcdefghi",
    "link",
    "subdir",
  ]);
});

Deno.test("expandGlob() accepts extended option set as true", async function () {
  const options = { ...EG_OPTIONS, extended: true };
  assertEquals(await expandGlobArray("abc?(def|ghi)", options), [
    "abc",
    "abcdef",
  ]);
  assertEquals(await expandGlobArray("abc*(def|ghi)", options), [
    "abc",
    "abcdef",
    "abcdefghi",
  ]);
  assertEquals(await expandGlobArray("abc+(def|ghi)", options), [
    "abcdef",
    "abcdefghi",
  ]);
  assertEquals(await expandGlobArray("abc@(def|ghi)", options), ["abcdef"]);
  assertEquals(await expandGlobArray("abc{def,ghi}", options), ["abcdef"]);
  assertEquals(await expandGlobArray("abc!(def|ghi)", options), ["abc"]);
});

Deno.test("expandGlobSync() accepts extended option set as true", function () {
  const options = { ...EG_OPTIONS, extended: true };
  assertEquals(expandGlobSyncArray("abc?(def|ghi)", options), [
    "abc",
    "abcdef",
  ]);
  assertEquals(expandGlobSyncArray("abc*(def|ghi)", options), [
    "abc",
    "abcdef",
    "abcdefghi",
  ]);
  assertEquals(expandGlobSyncArray("abc+(def|ghi)", options), [
    "abcdef",
    "abcdefghi",
  ]);
  assertEquals(expandGlobSyncArray("abc@(def|ghi)", options), ["abcdef"]);
  assertEquals(expandGlobSyncArray("abc{def,ghi}", options), ["abcdef"]);
  assertEquals(expandGlobSyncArray("abc!(def|ghi)", options), ["abc"]);
});

Deno.test("expandGlob() with globstar returns all dirs", async function () {
  const options = { ...EG_OPTIONS };
  assertEquals(
    await expandGlobArray("**/abc", options),
    ["abc", join("subdir", "abc")],
  );
});

Deno.test("expandGlobSync() with globstar returns all dirs", function () {
  const options = { ...EG_OPTIONS };
  assertEquals(
    expandGlobSyncArray("**/abc", options),
    ["abc", join("subdir", "abc")],
  );
});

Deno.test("expandGlob() with globstar parent returns all dirs", async function () {
  const options = { ...EG_OPTIONS, globstar: true };
  assertEquals(
    await expandGlobArray(joinGlobs(["subdir", "**", ".."], options), options),
    ["."],
  );
});

Deno.test("expandGlobSync() with globstar parent returns all dirs", function () {
  const options = { ...EG_OPTIONS, globstar: true };
  assertEquals(
    expandGlobSyncArray(joinGlobs(["subdir", "**", ".."], options), options),
    ["."],
  );
});

Deno.test("expandGlob() with globstar parent and globstar option set to false returns current dir", async function () {
  const options = { ...EG_OPTIONS, globstar: false };
  assertEquals(await expandGlobArray("**", options), [
    ".",
    "a[b]c",
    "abc",
    "abcdef",
    "abcdefghi",
    "link",
    "subdir",
  ]);
});

Deno.test("expandGlobSync() with globstar parent and globstar option set to false returns current dir", function () {
  const options = { ...EG_OPTIONS, globstar: false };
  assertEquals(expandGlobSyncArray("**", options), [
    ".",
    "a[b]c",
    "abc",
    "abcdef",
    "abcdefghi",
    "link",
    "subdir",
  ]);
});

Deno.test("expandGlob() accepts includeDirs option set to false", async function () {
  const options = { ...EG_OPTIONS, includeDirs: false };
  assertEquals(await expandGlobArray("subdir", options), []);
});

Deno.test("expandGlobSync() accepts includeDirs option set to false", function () {
  const options = { ...EG_OPTIONS, includeDirs: false };
  assertEquals(expandGlobSyncArray("subdir", options), []);
});

Deno.test("expandGlob() throws permission error without fs permissions", async function () {
  const exampleUrl = new URL("testdata/expand_wildcard.js", import.meta.url);
  const command = new Deno.Command(Deno.execPath(), {
    args: ["run", "--quiet", "--unstable", exampleUrl.toString()],
  });
  const { code, success, stdout, stderr } = await command.output();
  const decoder = new TextDecoder();
  assert(!success);
  assertEquals(code, 1);
  assertEquals(decoder.decode(stdout), "");
  assertStringIncludes(decoder.decode(stderr), "Uncaught PermissionDenied");
});

Deno.test("expandGlob() returns single entry when root is not glob", async function () {
  const options = { ...EG_OPTIONS, root: join(EG_OPTIONS.root!, "a[b]c") };
  assertEquals(await expandGlobArray("*", options), ["foo"]);
});

Deno.test("expandGlobSync() returns single entry when root is not glob", function () {
  const options = { ...EG_OPTIONS, root: join(EG_OPTIONS.root!, "a[b]c") };
  assertEquals(expandGlobSyncArray("*", options), ["foo"]);
});

Deno.test("expandGlob() accepts followSymlinks option set to true", async function () {
  const options = {
    ...EG_OPTIONS,
    root: join(EG_OPTIONS.root!, "link"),
    followSymlinks: true,
  };
  assertEquals(await expandGlobArray("*", options), ["abc"]);
});

Deno.test("expandGlobSync() accepts followSymlinks option set to true", function () {
  const options = {
    ...EG_OPTIONS,
    root: join(EG_OPTIONS.root!, "link"),
    followSymlinks: true,
  };
  assertEquals(expandGlobSyncArray("*", options), ["abc"]);
});

Deno.test("expandGlob() accepts followSymlinks option set to true with canonicalize", async function () {
  const options = {
    ...EG_OPTIONS,
    root: join(EG_OPTIONS.root!, "."),
    followSymlinks: true,
  };
  assertEquals(
    await expandGlobArray("**/abc", options),
    ["abc", join("subdir", "abc")],
  );
});

Deno.test("expandGlobSync() accepts followSymlinks option set to true with canonicalize", function () {
  const options = {
    ...EG_OPTIONS,
    root: join(EG_OPTIONS.root!, "."),
    followSymlinks: true,
  };
  assertEquals(
    expandGlobSyncArray("**/abc", options),
    ["abc", join("subdir", "abc")],
  );
});

Deno.test("expandGlob() accepts followSymlinks option set to true without canonicalize", async function () {
  const options = {
    ...EG_OPTIONS,
    root: join(EG_OPTIONS.root!, "."),
    followSymlinks: true,
    canonicalize: false,
  };
  assertEquals(
    await expandGlobArray("**/abc", options),
    ["abc", join("link", "abc"), join("subdir", "abc")],
  );
});

Deno.test("expandGlobSync() accepts followSymlinks option set to true without canonicalize", function () {
  const options = {
    ...EG_OPTIONS,
    root: join(EG_OPTIONS.root!, "."),
    followSymlinks: true,
    canonicalize: false,
  };
  assertEquals(
    expandGlobSyncArray("**/abc", options),
    ["abc", join("link", "abc"), join("subdir", "abc")],
  );
});

Deno.test(
  "expandGlob() does not require read permissions when root path is specified",
  {
    permissions: { read: [EG_OPTIONS.root!] },
  },
  async function () {
    const options = { root: EG_OPTIONS.root! };
    assertEquals(await expandGlobArray("abc", options), ["abc"]);
  },
);

Deno.test(
  "expandGlobSync() does not require read permissions when root path is specified",
  {
    permissions: { read: [EG_OPTIONS.root!] },
  },
  function () {
    const options = { root: EG_OPTIONS.root! };
    assertEquals(expandGlobSyncArray("abc", options), ["abc"]);
  },
);

Deno.test(
  "expandGlob() does not require read permissions when an absolute glob is specified",
  {
    permissions: { read: [EG_OPTIONS.root!] },
  },
  async function () {
    assertEquals(
      await expandGlobArray(`${EG_OPTIONS.root!}/abc`, {}, {
        forceRoot: EG_OPTIONS.root!,
      }),
      ["abc"],
    );
  },
);

Deno.test(
  "expandGlobSync() does not require read permissions when an absolute glob is specified",
  {
    permissions: { read: [EG_OPTIONS.root!] },
  },
  function () {
    assertEquals(
      expandGlobSyncArray(`${EG_OPTIONS.root!}/abc`, {}, {
        forceRoot: EG_OPTIONS.root!,
      }),
      ["abc"],
    );
  },
);
