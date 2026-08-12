// A workspace member (package.json) importing another workspace member by its
// package name, where the "exports" map points at a TypeScript file. The
// compiled binary must honor the exports map and transpile the target.
// Regression test for #27315.
import { a } from "@my-scope/lib-a";

console.log(a);
