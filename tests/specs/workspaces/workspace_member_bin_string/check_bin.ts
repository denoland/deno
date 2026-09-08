// Verifies the string form of a workspace member's `bin`
// (`"bin": "./cli.js"`), which takes its entry name from the package name —
// and for a scoped package that means the name *without* the scope.

/** See `workspace_member_bin/check_bin.ts` — on Windows the entry is a shim
 * file that embeds `$basedir`-relative paths rather than a symlink. */
function binTargets(binPath: string): string[] {
  if (Deno.build.os !== "windows") {
    Deno.lstatSync(binPath);
    return [binPath];
  }
  const dir = binPath.slice(0, binPath.lastIndexOf("/"));
  const text = Deno.readTextFileSync(binPath);
  return [...text.matchAll(/\$basedir\/([^"]+)/g)].map((m) => `${dir}/${m[1]}`);
}

function assertBinTarget(binPath: string, expectedFile: string) {
  const expected = Deno.realPathSync(expectedFile);
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

function assertMissing(path: string) {
  try {
    Deno.lstatSync(path);
  } catch (err) {
    if (err instanceof Deno.errors.NotFound) {
      return;
    }
    throw err;
  }
  throw new Error(`expected ${path} not to exist`);
}

// `@scope/tool` -> `.bin/tool`: the scope is dropped, and no `@scope`
// directory is created inside `.bin`.
assertBinTarget("node_modules/.bin/tool", "packages/scoped-tool/cli.js");
assertMissing("node_modules/.bin/@scope");

// an unscoped package keeps its full name
assertBinTarget("node_modules/.bin/plain-tool", "packages/plain-tool/main.js");

console.log("ok");
