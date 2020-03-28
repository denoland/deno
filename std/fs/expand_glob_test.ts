const { cwd, execPath, run } = Deno;
import { decode } from "../strings/mod.ts";
import { assert, assertEquals, assertStrContains } from "../testing/asserts.ts";
import {
  isWindows,
  join,
  joinGlobs,
  normalize,
  relative,
} from "../path/mod.ts";
import {
  ExpandGlobOptions,
  expandGlob,
  expandGlobSync,
} from "./expand_glob.ts";

async function expandGlobArray(
  globString: string,
  options: ExpandGlobOptions
): Promise<string[]> {
  const paths: string[] = [];
  for await (const { filename } of expandGlob(globString, options)) {
    paths.push(filename);
  }
  paths.sort();
  const pathsSync = [...expandGlobSync(globString, options)].map(
    ({ filename }): string => filename
  );
  pathsSync.sort();
  assertEquals(paths, pathsSync);
  const root = normalize(options.root || cwd());
  for (const path of paths) {
    assert(path.startsWith(root));
  }
  const relativePaths = paths.map(
    (path: string): string => relative(root, path) || "."
  );
  relativePaths.sort();
  return relativePaths;
}

function urlToFilePath(url: URL): string {
  // Since `new URL('file:///C:/a').pathname` is `/C:/a`, remove leading slash.
  return url.pathname.slice(url.protocol == "file:" && isWindows ? 1 : 0);
}

const EG_OPTIONS: ExpandGlobOptions = {
  root: urlToFilePath(new URL(join("testdata", "glob"), import.meta.url)),
  includeDirs: true,
  extended: false,
  globstar: false,
};

Deno.test(async function expandGlobWildcard(): Promise<void> {
  const options = EG_OPTIONS;
  assertEquals(await expandGlobArray("*", options), [
    "abc",
    "abcdef",
    "abcdefghi",
    "subdir",
  ]);
});

Deno.test(async function expandGlobTrailingSeparator(): Promise<void> {
  const options = EG_OPTIONS;
  assertEquals(await expandGlobArray("*/", options), ["subdir"]);
});

Deno.test(async function expandGlobParent(): Promise<void> {
  const options = EG_OPTIONS;
  assertEquals(await expandGlobArray("subdir/../*", options), [
    "abc",
    "abcdef",
    "abcdefghi",
    "subdir",
  ]);
});

Deno.test(async function expandGlobExt(): Promise<void> {
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

Deno.test(async function expandGlobGlobstar(): Promise<void> {
  const options = { ...EG_OPTIONS, globstar: true };
  assertEquals(
    await expandGlobArray(joinGlobs(["**", "abc"], options), options),
    ["abc", join("subdir", "abc")]
  );
});

Deno.test(async function expandGlobGlobstarParent(): Promise<void> {
  const options = { ...EG_OPTIONS, globstar: true };
  assertEquals(
    await expandGlobArray(joinGlobs(["subdir", "**", ".."], options), options),
    ["."]
  );
});

Deno.test(async function expandGlobIncludeDirs(): Promise<void> {
  const options = { ...EG_OPTIONS, includeDirs: false };
  assertEquals(await expandGlobArray("subdir", options), []);
});

Deno.test(async function expandGlobPermError(): Promise<void> {
  const exampleUrl = new URL("testdata/expand_wildcard.js", import.meta.url);
  const p = run({
    cmd: [execPath(), exampleUrl.toString()],
    stdin: "null",
    stdout: "piped",
    stderr: "piped",
  });
  assertEquals(await p.status(), { code: 1, success: false });
  assertEquals(decode(await p.output()), "");
  assertStrContains(
    decode(await p.stderrOutput()),
    "Uncaught PermissionDenied"
  );
  p.close();
});
