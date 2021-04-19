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
      ASCII_DIGIT: string[];
      ASCII_UPPER_ALPHA: string[];
      ASCII_LOWER_ALPHA: string[];
      ASCII_ALPHA: string[];
      ASCII_ALPHANUMERIC: string[];
      HTTP_TAB_OR_SPACE: string[];
      HTTP_WHITESPACE: string[];
      HTTP_TOKEN_CODE_POINT: string[];
      HTTP_TOKEN_CODE_POINT_RE: RegExp;
      HTTP_QUOTED_STRING_TOKEN_POINT: string[];
      HTTP_QUOTED_STRING_TOKEN_POINT_RE: RegExp;
      HTTP_WHITESPACE_PREFIX_RE: RegExp;
      HTTP_WHITESPACE_SUFFIX_RE: RegExp;
      regexMatcher(chars: string[]): string;
      byteUpperCase(s: string): string;
      byteLowerCase(s: string): string;
    };

    declare namespace mimesniff {
      declare interface MimeType {
        type: string;
        subtype: string;
        parameters: Map<string, string>;
      }
      declare function parseMimeType(input: string): MimeType | null;
    }

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
