// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

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

    declare var streams: {
      ReadableStream: typeof ReadableStream;
      isReadableStreamDisturbed(stream: ReadableStream): boolean;
    };
  }
}
