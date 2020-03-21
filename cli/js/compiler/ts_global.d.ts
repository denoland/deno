// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

// This scopes the `ts` namespace globally, which is where it exists at runtime
// when building Deno, but the `typescript/lib/typescript.d.ts` is defined as a
// module.

// Warning! This is a magical import. We don't want to have multiple copies of
// typescript.d.ts around the repo, there's already one in
// deno_typescript/typescript/lib/typescript.d.ts. Ideally we could simply point
// to that in this import specifier, but "cargo package" is very strict and
// requires all files to be present in a crate's subtree.
// eslint-disable-next-line @typescript-eslint/no-unused-vars
import * as ts_ from "$asset$/typescript.d.ts";

declare global {
  namespace ts {
    export = ts_;
  }

  namespace ts {
    // this are marked @internal in TypeScript, but we need to access them,
    // there is a risk these could change in future versions of TypeScript
    export const libs: string[];
    export const libMap: Map<string, string>;
  }
}
