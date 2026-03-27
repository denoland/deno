// Simulates what a BDD library (e.g. @std/testing/bdd) does when it wraps
// user tests: it calls Deno.test() internally but sets
// Symbol.for("Deno.test.location") on the test function so the runner uses
// the user's source location instead of the library's own file.

// deno-lint-ignore no-explicit-any
const libraryRegisteredFn: any = () => {};
libraryRegisteredFn[Symbol.for("Deno.test.location")] =
  "https://jsr.io/@std/testing/1.0.0/bdd.ts:100:10";
Deno.test("library registered test", libraryRegisteredFn);

// Ordinary test without a location override — location is this file.
Deno.test("normal test", () => {});
