// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.

// This scopes the `ts` namespace globally, which is where it exists at runtime
// when building Deno, but the `typescript/lib/typescript.d.ts` is defined as a
// module

import * as _ts from "typescript";

declare global {
  namespace ts {
    export = _ts;
  }
}
