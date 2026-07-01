// The root project imports a linked package by bare name, and that linked
// package in turn imports a sibling linked package by bare name. This is the
// transitivity case from issue #35214 that an import map entry in the root
// deno.json cannot satisfy, because the root import map does not apply inside
// the linked package's own scope.
import { a } from "@scope/aaa";

console.log(a());
