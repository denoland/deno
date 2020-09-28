# Import and export modules

## Concepts

- [import](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Statements/import)
  allows you to include and use modules held elsewhere, on your local file
  system or remotely.
- Imports are URLs or file system paths.
- [export](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Statements/export)
  allows you to specify which parts of your module are accessible to users who
  import your module.

## Overview

Deno by default standardizes the way modules are imported in both JavaScript and
TypeScript using the ECMAScript 6 `import/export` standard.

It adopts browser-like module resolution, meaning that file names must be
specified in full. You may not omit the file extension and there is no special
handling of `index.js`.

```js
import { add, multiply } from "./arithmetic.ts";
```

Dependencies are also imported directly, there is no package management
overhead. Local modules are imported in exactly the same way as remote modules.
As the examples show below, the same functionality can be produced in the same
way with local or remote modules.

## Local Import

In this example the `add` and `multiply` functions are imported from a local
`arithmetic.ts` module.

**Command:** `deno run local.ts`

```ts
/**
 * local.ts
 */
import { add, multiply } from "./arithmetic.ts";

function totalCost(outbound: number, inbound: number, tax: number): number {
  return multiply(add(outbound, inbound), tax);
}

console.log(totalCost(19, 31, 1.2));
console.log(totalCost(45, 27, 1.15));

/**
 * Output
 *
 * 60
 * 82.8
 */
```

## Remote Import

In the local import example above an `add` and `multiply` method are imported
from a locally stored arithmetic module. The same functionality can be created
by importing `add` and `multiply` methods from a remote module too.

In this case the Ramda module is referenced, including the version number. Also
note a JavaScript module is imported directly into a TypeSript module, Deno has
no problem handling this.

**Command:** `deno run ./remote.ts`

```ts
/**
 * remote.ts
 */
import {
  add,
  multiply,
} from "https://x.nest.land/ramda@0.27.0/source/index.js";

function totalCost(outbound: number, inbound: number, tax: number): number {
  return multiply(add(outbound, inbound), tax);
}

console.log(totalCost(19, 31, 1.2));
console.log(totalCost(45, 27, 1.15));

/**
 * Output
 *
 * 60
 * 82.8
 */
```

## Export

In the local import example above the `add` and `multiply` functions are
imported from a locally stored arithmetic module. To make this possible the
functions stored in the arithmetic module must be exported.

To do this just add the keyword `export` to the beginning of the function
signature as is shown below.

```ts
/**
 * arithmetic.ts
 */
export function add(a: number, b: number): number {
  return a + b;
}

export function multiply(a: number, b: number): number {
  return a * b;
}
```

All functions, classes, constants and variables which need to be accessible
inside external modules must be exported. Either by prepending them with the
`export` keyword or including them in an export statement at the bottom of the
file.
