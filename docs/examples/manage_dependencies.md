# Managing dependencies

## Concepts

- Deno uses URLs for dependency management.
- One convention places all these dependent URLs into a local `deps.ts` file.
  Functionality is then exported out of `deps.ts` for use by local modules.
- Continuing this convention, dev only dependencies can be kept in a
  `dev_deps.ts` file.
- See also [Linking to external code](../linking_to_external_code.md)

## Overview

In Deno there is no concept of a package manager as external modules are
imported directly into local modules. This raises the question of how to manage
remote dependencies without a package manager. In big projects with many
dependencies it will become cumbersome and time consuming to update modules if
they are all imported individually into individual modules.

The standard practice for solving this problem in Deno is to create a `deps.ts`
file. All required remote dependencies are referenced in this file and the
required methods and classes are re-exported. The dependent local modules then
reference the `deps.ts` rather than the remote dependencies. If now for example
one remote dependency is used in several files, upgrading to a new version of
this remote dependency is much simpler as this can be done just within
`deps.ts`.

With all dependencies centralized in `deps.ts`, managing these becomes easier.
Dev dependencies can also be managed in a separate `dev_deps.ts` file, allowing
clean separation between dev only and production dependencies.

## Example

```ts
/**
 * deps.ts
 *
 * This module re-exports the required methods from the dependant remote Ramda module.
 **/
export {
  add,
  multiply,
} from "https://x.nest.land/ramda@0.27.0/source/index.js";
```

In this example the same functionality is created as is the case in the
[local and remote import examples](./import_export.md). But in this case instead
of the Ramda module being referenced directly it is referenced by proxy using a
local `deps.ts` module.

**Command:** `deno run example.ts`

```ts
/**
 * example.ts
 */

import { add, multiply } from "./deps.ts";

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
