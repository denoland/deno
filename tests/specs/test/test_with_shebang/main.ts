// `deno test --doc` tries to convert the example code snippets into pseudo
// test files in a way that all the exported items are available without
// explicit import statements. Therefore, in the test code, you don't have to
// write like `import { add } from "./main.ts";`.
// However, this automatic import resolution might conflict with other
// explicitly declared identifiers in the test code you write. This spec test
// makes sure that such cases will not cause any issues - explicit identifiers
// take precedence.

/**
 * ```ts
 * #!/usr/bin/env -S deno run --allow-read
 * import { assertEquals } from "@std/assert";
 * import { getModuleName, createFoo } from "./mod.ts";
 *
 * const foo = createFoo();
 * assertEquals(getModuleName(), "mod.ts");
 * assertEquals(add(1, 2), foo());
 * ```
 */
export function add(a: number, b: number): number {
  return a + b;
}
