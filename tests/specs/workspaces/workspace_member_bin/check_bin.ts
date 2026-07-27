// Verifies that a local workspace member which declares a `bin` in its
// package.json gets a `node_modules/.bin` entry, both in the workspace root
// and in a sibling member that depends on it. Previously only external npm
// dependencies got these, so `deno task` could not invoke a local member's
// executable. https://github.com/denoland/deno/issues/36313

function assertBinEntry(path: string) {
  try {
    // lstat (not stat) because on unix the entry is a symlink into the member
    // and on windows it is a plain shim file.
    Deno.lstatSync(path);
  } catch (err) {
    if (err instanceof Deno.errors.NotFound) {
      throw new Error(`expected ${path} to exist`);
    }
    throw err;
  }
}

// (a) The root `node_modules/.bin` has the member's executable, so tooling
// (and a root task) can run it.
assertBinEntry("node_modules/.bin/local-cli");

// (b) The depending member's own `node_modules/.bin` has it too, mirroring how
// npm and pnpm lay out workspaces.
assertBinEntry("apps/web/node_modules/.bin/local-cli");

console.log("ok");
