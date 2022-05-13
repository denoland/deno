// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

// deno-lint-ignore-file no-var

/// <reference no-default-lib="true" />
/// <reference lib="esnext" />

declare namespace globalThis {
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
      HTTP_TAB_OR_SPACE_PREFIX_RE: RegExp;
      HTTP_TAB_OR_SPACE_SUFFIX_RE: RegExp;
      HTTP_WHITESPACE_PREFIX_RE: RegExp;
      HTTP_WHITESPACE_SUFFIX_RE: RegExp;
      httpTrim(s: string): string;
      regexMatcher(chars: string[]): string;
      byteUpperCase(s: string): string;
      byteLowerCase(s: string): string;
      collectHttpQuotedString(
        input: string,
        position: number,
        extractValue: boolean,
      ): {
        result: string;
        position: number;
      };
      forgivingBase64Encode(data: Uint8Array): string;
      forgivingBase64Decode(data: string): Uint8Array;
      serializeJSValueToJSONString(value: unknown): string;
    };

    declare var domException: {
      DOMException: typeof DOMException;
    };

    declare namespace mimesniff {
      declare interface MimeType {
        type: string;
        subtype: string;
        parameters: Map<string, string>;
      }
      declare function parseMimeType(input: string): MimeType | null;
      declare function essence(mimeType: MimeType): string;
      declare function serializeMimeType(mimeType: MimeType): string;
      declare function extractMimeType(
        headerValues: string[] | null,
      ): MimeType | null;
    }

    declare var eventTarget: {
      EventTarget: typeof EventTarget;
    };

    declare var location: {
      getLocationHref(): string | undefined;
    };

    declare var base64: {
      atob(data: string): string;
      btoa(data: string): string;
    };

    declare var file: {
      blobFromObjectUrl(url: string): Blob | null;
      getParts(blob: Blob): string[];
      Blob: typeof Blob;
      File: typeof File;
    };

    declare var streams: {
      ReadableStream: typeof ReadableStream;
      isReadableStreamDisturbed(stream: ReadableStream): boolean;
      createProxy<T>(stream: ReadableStream<T>): ReadableStream<T>;
    };

    declare namespace messagePort {
      declare type Transferable = {
        kind: "messagePort";
        data: number;
      } | {
        kind: "arrayBuffer";
        data: number;
      };
      declare interface MessageData {
        data: Uint8Array;
        transferables: Transferable[];
      }
    }
  }
}
