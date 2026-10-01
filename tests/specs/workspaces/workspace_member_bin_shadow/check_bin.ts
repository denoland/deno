// Sanity check for the fixture the task steps rely on: the two `foo` entries
// must actually point at different members, otherwise the precedence
// assertions below would pass for the wrong reason.

function assertBinTarget(binPath: string, expectedFile: string) {
  const expected = Deno.realPathSync(expectedFile);
  let resolved: string;
  try {
    resolved = Deno.realPathSync(binPath);
  } catch {
    throw new Error(`expected ${binPath} to exist`);
  }
  if (resolved !== expected) {
    throw new Error(
      `expected ${binPath} to point at ${expected}, but got ${resolved}`,
    );
  }
}

// `z-js` wins the root entry: both members sort to depth `u64::MAX` and the
// tiebreak is a descending `<name>@<version>` compare.
assertBinTarget("node_modules/.bin/foo", "packages/z-js/cli.js");
// `apps/web` only depends on `a-sh`, so its own `.bin` has the shell script.
assertBinTarget("apps/web/node_modules/.bin/foo", "packages/a-sh/tool.sh");

console.log("ok");
