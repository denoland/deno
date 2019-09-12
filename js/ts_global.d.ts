// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.

// This scopes the `ts` namespace globally, which is where it exists at runtime
// when building Deno, but the `typescript/lib/typescript.d.ts` is defined as a
// module.

// eslint-disable-next-line @typescript-eslint/no-unused-vars
import * as ts_ from "../node_modules/typescript/lib/typescript.d.ts";

declare global {
  namespace ts {
    export = ts_;
  }
}
