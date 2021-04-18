// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

/// <reference no-default-lib="true" />
/// <reference lib="esnext" />

declare namespace globalThis {
  declare var TextEncoder: typeof TextEncoder;
  declare var TextDecoder: typeof TextDecoder;

  declare namespace __bootstrap {
    declare var infra: {
      collectSequenceOfCodepoints(
        input: string,
        position: number,
        condition: (char: string) => boolean,
      ): {
        result: string;
        position: number;
      };
    };

    declare var mimesniff: {
      parseMimeType(input: string): {
        type: string;
        subtype: string;
        parameters: Map<string, string>;
      } | null;
    };

    declare var eventTarget: {
      EventTarget: typeof EventTarget;
    };

    declare var location: {
      getLocationHref(): string | undefined;
    };

    declare var base64: {
      byteLength(b64: string): number;
      toByteArray(b64: string): Uint8Array;
      fromByteArray(uint8: Uint8Array): string;
    };
  }
}
