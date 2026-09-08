// Verifies that a local workspace member which declares a `bin` in its
// package.json gets a `node_modules/.bin` entry, both in the workspace root
// and in a sibling member that depends on it. Previously only external npm
// dependencies got these, so `deno task` could not invoke a local member's
// executable. https://github.com/denoland/deno/issues/36313

/** The files a `node_modules/.bin` entry points at. On unix the entry is a
 * symlink into the member, so the entry path itself resolves. On Windows it is
 * a generated shim script that embeds `$basedir`-relative paths (one for the
 * interpreter, one for the script), so pull those out of the text. */
function binTargets(binPath: string): string[] {
  if (Deno.build.os !== "windows") {
    // touch the entry so a missing one reports as NotFound here
    Deno.lstatSync(binPath);
    return [binPath];
  }
  const dir = binPath.slice(0, binPath.lastIndexOf("/"));
  const text = Deno.readTextFileSync(binPath);
  return [...text.matchAll(/\$basedir\/([^"]+)/g)].map((m) => `${dir}/${m[1]}`);
}

/** Asserts that the `.bin` entry actually resolves to `expectedFile`. Merely
 * existing isn't enough: a shim built from the wrong package path would look
 * identical on disk. */
function assertBinTarget(binPath: string, expectedFile: string) {
  let expected: string;
  try {
    expected = Deno.realPathSync(expectedFile);
  } catch {
    throw new Error(`expected bin script ${expectedFile} to exist`);
  }
  let candidates: string[];
  try {
    candidates = binTargets(binPath);
  } catch (err) {
    if (err instanceof Deno.errors.NotFound) {
      throw new Error(`expected ${binPath} to exist`);
    }
    throw err;
  }
  const resolved = candidates.map((path) => {
    try {
      return Deno.realPathSync(path);
    } catch {
      return null;
    }
  });
  if (!resolved.includes(expected)) {
    throw new Error(
      `expected ${binPath} to point at ${expected}, but it resolved to ${
        JSON.stringify(resolved)
      }`,
    );
  }
}

// (a) The root `node_modules/.bin` has the member's executable, so tooling
// (and a root task) can run it.
assertBinTarget("node_modules/.bin/local-cli", "packages/local-cli/cli.js");

// (b) The depending member's own `node_modules/.bin` has it too, mirroring how
// npm and pnpm lay out workspaces.
assertBinTarget(
  "apps/web/node_modules/.bin/local-cli",
  "packages/local-cli/cli.js",
);

// (c) A member whose bin is not a JavaScript file gets an entry all the same —
// it just has to keep resolving through `PATH` instead of being handed to
// `deno run`.
assertBinTarget("node_modules/.bin/shtool", "packages/shtool/tool.sh");

console.log("ok");
