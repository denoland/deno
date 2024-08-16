// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

/// <reference no-default-lib="true" />
/// <reference lib="esnext" />

declare module "ext:deno_console/01_console.js" {
  function privateInspect<TObject>(
    object: TObject,
    keys: (keyof TObject)[],
    // deno-lint-ignore no-explicit-any
    inspect: any,
    // deno-lint-ignore no-explicit-any
    inspectOptions: any,
  ): string;
}
