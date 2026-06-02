// Importing a non-Deno workspace member (package.json with an "exports" map) by
// its package name must honor the exports map in the compiled binary, both for
// the root export and a subpath export. Regression test for #28926.
import { greet } from "@repo/lib";
import { sum } from "@repo/lib/util";

console.log(greet());
console.log(sum(1, 2));
