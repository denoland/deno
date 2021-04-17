// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

// deno-lint-ignore-file no-explicit-any

/// <reference no-default-lib="true" />
/// <reference lib="esnext" />

declare namespace globalThis {
  declare namespace __bootstrap {
    declare var fetchUtil: {
      requiredArguments(name: string, length: number, required: number): void;
    };

    declare var domIterable: {
      DomIterableMixin(base: any, dataSymbol: symbol): any;
    };

    declare var headers: {
      Headers: typeof Headers;
    };

    declare var formData: {
      FormData: typeof FormData;
      encodeFormData(formdata: FormData): {
        body: Uint8Array;
        contentType: string;
      };
      parseFormData(body: Uint8Array, boundary: string | undefined): FormData;
    };

    declare var streams: {
      ReadableStream: typeof ReadableStream;
      isReadableStreamDisturbed(stream: ReadableStream): boolean;
    };
  }
}
