// Copyright 2018-2025 the Deno authors. MIT license.

/// <reference no-default-lib="true" />
/// <reference lib="esnext" />

declare module "ext:deno_console/01_console.js" {
  function createFilteredInspectProxy<TObject>(params: {
    object: TObject;
    keys: (keyof TObject)[];
    evaluate: boolean;
  }): Record<string, unknown>;

  class Console {
  }
}
