// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

/// <reference no-default-lib="true" />
/// <reference lib="esnext" />

declare module "ext:deno_web/00_infra.js" {
  function collectSequenceOfCodepoints(
    input: string,
    position: number,
    condition: (char: string) => boolean,
  ): {
    result: string;
    position: number;
  };
  const ASCII_DIGIT: string[];
  const ASCII_UPPER_ALPHA: string[];
  const ASCII_LOWER_ALPHA: string[];
  const ASCII_ALPHA: string[];
  const ASCII_ALPHANUMERIC: string[];
  const HTTP_TAB_OR_SPACE: string[];
  const HTTP_WHITESPACE: string[];
  const HTTP_TOKEN_CODE_POINT: string[];
  const HTTP_TOKEN_CODE_POINT_RE: RegExp;
  const HTTP_QUOTED_STRING_TOKEN_POINT: string[];
  const HTTP_QUOTED_STRING_TOKEN_POINT_RE: RegExp;
  const HTTP_TAB_OR_SPACE_PREFIX_RE: RegExp;
  const HTTP_TAB_OR_SPACE_SUFFIX_RE: RegExp;
  const HTTP_WHITESPACE_PREFIX_RE: RegExp;
  const HTTP_WHITESPACE_SUFFIX_RE: RegExp;
  function httpTrim(s: string): string;
  function regexMatcher(chars: string[]): string;
  function byteUpperCase(s: string): string;
  function byteLowerCase(s: string): string;
  function collectHttpQuotedString(
    input: string,
    position: number,
    extractValue: boolean,
  ): {
    result: string;
    position: number;
  };
  function forgivingBase64Encode(data: Uint8Array): string;
  function forgivingBase64Decode(data: string): Uint8Array;
  function forgivingBase64UrlEncode(data: Uint8Array | string): string;
  function forgivingBase64UrlDecode(data: string): Uint8Array;
  function pathFromURL(urlOrPath: string | URL): string;
  function serializeJSValueToJSONString(value: unknown): string;
}

declare module "ext:deno_web/01_dom_exception.js" {
  const DOMException: DOMException;
}

declare module "ext:deno_web/01_mimesniff.js" {
  interface MimeType {
    type: string;
    subtype: string;
    parameters: Map<string, string>;
  }
  function parseMimeType(input: string): MimeType | null;
  function essence(mimeType: MimeType): string;
  function serializeMimeType(mimeType: MimeType): string;
  function extractMimeType(
    headerValues: string[] | null,
  ): MimeType | null;
}

declare module "ext:deno_web/02_event.js" {
  const EventTarget: typeof EventTarget;
  const Event: typeof event;
  const ErrorEvent: typeof ErrorEvent;
  const CloseEvent: typeof CloseEvent;
  const MessageEvent: typeof MessageEvent;
  const CustomEvent: typeof CustomEvent;
  const ProgressEvent: typeof ProgressEvent;
  const PromiseRejectionEvent: typeof PromiseRejectionEvent;
  const reportError: typeof reportError;
}

declare module "ext:deno_web/12_location.js" {
  function getLocationHref(): string | undefined;
}

declare module "ext:deno_web/05_base64.js" {
  function atob(data: string): string;
  function btoa(data: string): string;
}

declare module "ext:deno_web/09_file.js" {
  function blobFromObjectUrl(url: string): Blob | null;
  function getParts(blob: Blob): string[];
  const Blob: typeof Blob;
  const File: typeof File;
}

declare module "ext:deno_web/06_streams.js" {
  const ReadableStream: typeof ReadableStream;
  function isReadableStreamDisturbed(stream: ReadableStream): boolean;
  function createProxy<T>(stream: ReadableStream<T>): ReadableStream<T>;
}

declare module "ext:deno_web/13_message_port.js" {
  type Transferable = {
    kind: "messagePort";
    data: number;
  } | {
    kind: "arrayBuffer";
    data: number;
  };
  interface MessageData {
    data: Uint8Array;
    transferables: Transferable[];
  }
  const MessageChannel: typeof MessageChannel;
  const MessagePort: typeof MessagePort;
  const MessagePortIdSymbol: typeof MessagePortIdSymbol;
  function deserializeJsMessageData(
    messageData: messagePort.MessageData,
  ): [object, object[]];
}
