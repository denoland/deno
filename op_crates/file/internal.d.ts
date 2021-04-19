// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

/// <reference no-default-lib="true" />
/// <reference lib="esnext" />

declare namespace globalThis {
  declare namespace __bootstrap {
    declare var file: {
      Blob: typeof Blob & {
        [globalThis.__bootstrap.file._byteSequence]: Uint8Array;
      };
      readonly _byteSequence: unique symbol;
      File: typeof File & {
        [globalThis.__bootstrap.file._byteSequence]: Uint8Array;
      };
    };
  }
}
