// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

/// <reference no-default-lib="true" />
/// <reference lib="esnext" />

declare namespace globalThis {
  declare namespace __bootstrap {
    declare namespace console {
      declare function createFilteredInspectProxy<TObject>(params: {
        object: TObject;
        keys: (keyof TObject)[];
        evaluate: boolean;
      }): Record<string, unknown>;
    }
  }
}
