// Regression test for the deny-bypass called out in review of
// https://github.com/denoland/deno/pull/34504.
//
// A recursive remove descends into the whole tree, so it must use strict deny
// semantics: removing `parent` recursively while `parent/denied` is denied must
// be rejected, and the denied subtree must survive.
try {
  Deno.removeSync("parent", { recursive: true });
  console.log("recursive remove parent: UNEXPECTED OK");
} catch (e) {
  console.log("recursive remove parent:", (e as Error).name);
}

// The denied subtree (and its contents) must still exist.
console.log(
  "parent/denied exists:",
  Deno.statSync("parent/denied").isDirectory,
);
console.log(
  "parent/denied/keep.txt:",
  Deno.readTextFileSync("parent/denied/keep.txt"),
);

// Removing a subtree that has no denied descendant still works.
Deno.removeSync("parent/other.txt");
console.log("recursive remove allowed subtree: ok");
