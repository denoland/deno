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

The triple backticks start and end code blocks. If this were in a file named
foo.ts, running deno test --doc foo.ts will extract this example, and then
type-check it as a standalone module.
