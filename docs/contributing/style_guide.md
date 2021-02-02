# Deno Style Guide

## Copyright Headers

Most modules in the repository should have the following copyright header:

```ts
// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
```

If the code originates elsewhere, ensure that the file has the proper copyright
headers. We only allow MIT, BSD, and Apache licensed code.

## Use underscores, not dashes in filenames.

Example: Use `file_server.ts` instead of `file-server.ts`.

## Add tests for new features.

Each module should contain or be accompanied by tests for its public
functionality.

## TODO Comments

TODO comments should usually include an issue or the author's github username in
parentheses. Example:

```ts
// TODO(ry): Add tests.
// TODO(#123): Support Windows.
// FIXME(#349): Sometimes panics.
```

## Meta-programming is discouraged. Including the use of Proxy.

Be explicit even when it means more code.

There are some situations where it may make sense to use such techniques, but in
the vast majority of cases it does not.

## Inclusive code

Please follow the guidelines for inclusive code outlined at
https://chromium.googlesource.com/chromium/src/+/master/styleguide/inclusive_code.md.

## Rust

Follow Rust conventions and be consistent with existing code.

## TypeScript

The TypeScript portions of the codebase include `cli/js` for the built-ins and
the standard library `std`.

### Use TypeScript instead of JavaScript.

### Use the term "module" instead of "library" or "package".

For clarity and consistency avoid the terms "library" and "package". Instead use
"module" to refer to a single JS or TS file and also to refer to a directory of
TS/JS code.

### Do not use the filename `index.ts`/`index.js`.

Deno does not treat "index.js" or "index.ts" in a special way. By using these
filenames, it suggests that they can be left out of the module specifier when
they cannot. This is confusing.

If a directory of code needs a default entry point, use the filename `mod.ts`.
The filename `mod.ts` follows Rust’s convention, is shorter than `index.ts`, and
doesn’t come with any preconceived notions about how it might work.

### Exported functions: max 2 args, put the rest into an options object.

When designing function interfaces, stick to the following rules.

1. A function that is part of the public API takes 0-2 required arguments, plus
   (if necessary) an options object (so max 3 total).

2. Optional parameters should generally go into the options object.

   An optional parameter that's not in an options object might be acceptable if
   there is only one, and it seems inconceivable that we would add more optional
   parameters in the future.

3. The 'options' argument is the only argument that is a regular 'Object'.

   Other arguments can be objects, but they must be distinguishable from a
   'plain' Object runtime, by having either:

   - a distinguishing prototype (e.g. `Array`, `Map`, `Date`, `class MyThing`).
   - a well-known symbol property (e.g. an iterable with `Symbol.iterator`).

   This allows the API to evolve in a backwards compatible way, even when the
   position of the options object changes.

```ts
// BAD: optional parameters not part of options object. (#2)
export function resolve(
  hostname: string,
  family?: "ipv4" | "ipv6",
  timeout?: number
): IPAddress[] {}

// GOOD.
export interface ResolveOptions {
  family?: "ipv4" | "ipv6";
  timeout?: number;
}
export function resolve(
  hostname: string,
  options: ResolveOptions = {}
): IPAddress[] {}
```

```ts
export interface Environment {
  [key: string]: string;
}

// BAD: `env` could be a regular Object and is therefore indistinguishable
// from an options object. (#3)
export function runShellWithEnv(cmdline: string, env: Environment): string {}

// GOOD.
export interface RunShellOptions {
  env: Environment;
}
export function runShellWithEnv(
  cmdline: string,
  options: RunShellOptions
): string {}
```

```ts
// BAD: more than 3 arguments (#1), multiple optional parameters (#2).
export function renameSync(
  oldname: string,
  newname: string,
  replaceExisting?: boolean,
  followLinks?: boolean
) {}

// GOOD.
interface RenameOptions {
  replaceExisting?: boolean;
  followLinks?: boolean;
}
export function renameSync(
  oldname: string,
  newname: string,
  options: RenameOptions = {}
) {}
```

```ts
// BAD: too many arguments. (#1)
export function pwrite(
  fd: number,
  buffer: TypedArray,
  offset: number,
  length: number,
  position: number
) {}

// BETTER.
export interface PWrite {
  fd: number;
  buffer: TypedArray;
  offset: number;
  length: number;
  position: number;
}
export function pwrite(options: PWrite) {}
```

### Use `interface` over `type` for creating object typings

Let's say, for example, you want to create an options object that must match a
specific TypeScript object shape for your `walkDir` function. Instead of using the
TypeScript `type` declaration like the following:

```ts
// BAD: Using `type` keyword!
export type WalkDirOptions = {
  recursive?: boolean;
};

export async function walkDir(directory: string, options: WalkDirOptions) {
  /* ... */
}
```

You should be defining `WalkDirOptions` with the `interface` keyword like in
this code example:

```ts
// GOOD: Using `interface` keyword!
export interface WalkDirOptions {
  recursive?: boolean;
}

export async function walkDir(directory: string, options: WalkDirOptions) {
  /* ... */
}
```

### Export all interfaces that are used as parameters to an exported member

Whenever you are using interfaces that are included in the arguments of an
exported member, you should export the interface that is used. Here is an
example:

```ts
// my_file.ts
export interface Person {
  name: string;
  age: number;
}

export function createPerson(name: string, age: number): Person {
  return { name, age };
}

// mod.ts
export { createPerson } from "./my_file.ts";
export type { Person } from "./my_file.ts";
```

### Minimize dependencies; do not make circular imports.

Although `cli/js` and `std` have no external dependencies, we must still be
careful to keep internal dependencies simple and manageable. In particular, be
careful not to introduce circular imports.

### If a filename starts with an underscore: `_foo.ts`, do not link to it.

Sometimes there may be situations where an internal module is necessary but its
API is not meant to be stable or linked to. In this case prefix it with an
underscore. By convention, only files in its own directory should import it.

### Use JSDoc for exported symbols.

We strive for complete documentation. Every exported symbol ideally should have
a documentation line.

If possible, use a single line for the JSDoc. Example:

```ts
/** foo does bar. */
export function foo() {
  // ...
}
```

It is important that documentation is easily human readable, but there is also a
need to provide additional styling information to ensure generated documentation
is more rich text. Therefore JSDoc should generally follow markdown markup to
enrich the text.

While markdown supports HTML tags, it is forbidden in JSDoc blocks.

Code string literals should be braced with the back-tick (\`) instead of quotes.
For example:

```ts
/** Import something from the `deno` module. */
```

Do not document function arguments unless they are non-obvious of their intent
(though if they are non-obvious intent, the API should be considered anyways).
Therefore `@param` should generally not be used. If `@param` is used, it should
not include the `type` as TypeScript is already strongly typed.

```ts
/**
 * Function with non obvious param.
 * @param foo Description of non obvious parameter.
 */
```

Vertical spacing should be minimized whenever possible. Therefore single line
comments should be written as:

```ts
/** This is a good single line JSDoc. */
```

And not:

```ts
/**
 * This is a bad single line JSDoc.
 */
```

Code examples should not utilise the triple-back tick (\`\`\`) notation or tags.
They should just be marked by indentation, which requires a break before the
block and 6 additional spaces for each line of the example. This is 4 more than
the first column of the comment. For example:

```ts
/** A straight forward comment and an example:
 *
 *       import { foo } from "deno";
 *       foo("bar");
 */
```

Code examples should not contain additional comments. It is already inside a
comment. If it needs further comments it is not a good example.

### Resolve linting problems using directives

Currently, the building process uses `dlint` to validate linting problems in the
code. If the task requires code that is non-conformant to linter use
`deno-lint-ignore <code>` directive to suppress the warning.

```typescript
// deno-lint-ignore no-explicit-any
let x: any;
```

This ensures the continuous integration process doesn't fail due to linting
problems, but it should be used scarcely.

### Each module should come with a test module.

Every module with public functionality `foo.ts` should come with a test module
`foo_test.ts`. A test for a `cli/js` module should go in `cli/js/tests` due to
their different contexts, otherwise it should just be a sibling to the tested
module.

### Unit Tests should be explicit.

For a better understanding of the tests, function should be correctly named as
its prompted throughout the test command. Like:

```
test myTestFunction ... ok
```

Example of test:

```ts
import { assertEquals } from "https://deno.land/std@$STD_VERSION/testing/asserts.ts";
import { foo } from "./mod.ts";

Deno.test("myTestFunction", function () {
  assertEquals(foo(), { bar: "bar" });
});
```

### Top level functions should not use arrow syntax.

Top level functions should use the `function` keyword. Arrow syntax should be
limited to closures.

Bad:

```ts
export const foo = (): string => {
  return "bar";
};
```

Good:

```ts
export function foo(): string {
  return "bar";
}
```

### `std`

#### Do not depend on external code.

`https://deno.land/std/` is intended to be baseline functionality that all Deno
programs can rely on. We want to guarantee to users that this code does not
include potentially unreviewed third party code.

#### Document and maintain browser compatibility.

If a module is browser compatible, include the following in the JSDoc at the top
of the module:

```ts
// This module is browser compatible.
```

Maintain browser compatibility for such a module by either not using the global
`Deno` namespace or feature-testing for it. Make sure any new dependencies are
also browser compatible.
