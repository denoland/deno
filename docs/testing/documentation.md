# Documentation tests

Deno supports type-checking your documentation examples.

This makes sure that examples within your documentation are up to date and
working.

The basic idea is this:

````ts
/**
 * # Examples
 *
 * ```ts
 * const x = 42;
 * ```
 */
````

The triple backticks mark the start and end of code blocks.

If this example was in a file named foo.ts, running `deno test --doc foo.ts`
will extract this example, and then type-check it as a standalone module living
in the same directory as the module being documented.

To document your exports, import the module using a relative path specifier:

````ts
/**
 * # Examples
 *
 * ```ts
 * import { foo } from "./foo.ts";
 * ```
 */
export function foo(): string {
  return "foo";
}
````
