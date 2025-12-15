// Copyright 2018-2025 the Deno authors. MIT license.
// deno-lint-ignore-file no-explicit-any

/// <reference no-default-lib="true" />
/// <reference lib="esnext" />

declare module "ext:deno_web/00_webidl.js" {
  function makeException(
    ErrorType: any,
    message: string,
    prefix?: string,
    context?: string,
  ): any;
  interface IntConverterOpts {
    /**
     * Whether to throw if the number is outside of the acceptable values for
     * this type.
     */
    enforceRange?: boolean;
    /**
     * Whether to clamp this number to the acceptable values for this type.
     */
    clamp?: boolean;
  }
  interface StringConverterOpts {
    /**
     * Whether to treat `null` value as an empty string.
     */
    treatNullAsEmptyString?: boolean;
  }
  interface BufferConverterOpts {
    /**
     * Whether to allow `SharedArrayBuffer` (not just `ArrayBuffer`).
     */
    allowShared?: boolean;
  }
  const converters: {
    any(v: any): any;
    /**
     * Convert a value into a `boolean` (bool).
     */
    boolean(
      v: any,
      prefix?: string,
      context?: string,
      opts?: IntConverterOpts,
    ): boolean;
    /**
     * Convert a value into a `byte` (int8).
     */
    byte(
      v: any,
      prefix?: string,
      context?: string,
      opts?: IntConverterOpts,
    ): number;
    /**
     * Convert a value into a `octet` (uint8).
     */
    octet(
      v: any,
      prefix?: string,
      context?: string,
      opts?: IntConverterOpts,
    ): number;
    /**
     * Convert a value into a `short` (int16).
     */
    short(
      v: any,
      prefix?: string,
      context?: string,
      opts?: IntConverterOpts,
    ): number;
    /**
     * Convert a value into a `unsigned short` (uint16).
     */
    ["unsigned short"](
      v: any,
      prefix?: string,
      context?: string,
      opts?: IntConverterOpts,
    ): number;
    /**
     * Convert a value into a `long` (int32).
     */
    long(
      v: any,
      prefix?: string,
      context?: string,
      opts?: IntConverterOpts,
    ): number;
    /**
     * Convert a value into a `unsigned long` (uint32).
     */
    ["unsigned long"](
      v: any,
      prefix?: string,
      context?: string,
      opts?: IntConverterOpts,
    ): number;
    /**
     * Convert a value into a `long long` (int64).
     * **Note this is truncated to a JS number (53 bit precision).**
     */
    ["long long"](
      v: any,
      prefix?: string,
      context?: string,
      opts?: IntConverterOpts,
    ): number;
    /**
     * Convert a value into a `unsigned long long` (uint64).
     * **Note this is truncated to a JS number (53 bit precision).**
     */
    ["unsigned long long"](
      v: any,
      prefix?: string,
      context?: string,
      opts?: IntConverterOpts,
    ): number;
    /**
     * Convert a value into a `float` (f32).
     */
    float(v: any, prefix?: string, context?: string, opts?: any): number;
    /**
     * Convert a value into a `unrestricted float` (f32, infinity, or NaN).
     */
    ["unrestricted float"](
      v: any,
      prefix?: string,
      context?: string,
      opts?: any,
    ): number;
    /**
     * Convert a value into a `double` (f64).
     */
    double(v: any, prefix?: string, context?: string, opts?: any): number;
    /**
     * Convert a value into a `unrestricted double` (f64, infinity, or NaN).
     */
    ["unrestricted double"](
      v: any,
      prefix?: string,
      context?: string,
      opts?: any,
    ): number;
    /**
     * Convert a value into a `DOMString` (string).
     */
    DOMString(
      v: any,
      prefix?: string,
      context?: string,
      opts?: StringConverterOpts,
    ): string;
    /**
     * Convert a value into a `ByteString` (string with only u8 codepoints).
     */
    ByteString(
      v: any,
      prefix?: string,
      context?: string,
      opts?: StringConverterOpts,
    ): string;
    /**
     * Convert a value into a `USVString` (string with only valid non
     * surrogate Unicode code points).
     */
    USVString(
      v: any,
      prefix?: string,
      context?: string,
      opts?: StringConverterOpts,
    ): string;
    /**
     * Convert a value into an `object` (object).
     */
    object(v: any, prefix?: string, context?: string, opts?: any): object;
    /**
     * Convert a value into an `ArrayBuffer` (ArrayBuffer).
     */
    ArrayBuffer(
      v: any,
      prefix?: string,
      context?: string,
      opts?: BufferConverterOpts,
    ): ArrayBuffer;
    /**
     * Convert a value into a `DataView` (ArrayBuffer).
     */
    DataView(
      v: any,
      prefix?: string,
      context?: string,
      opts?: BufferConverterOpts,
    ): DataView;
    /**
     * Convert a value into a `Int8Array` (Int8Array).
     */
    Int8Array(
      v: any,
      prefix?: string,
      context?: string,
      opts?: BufferConverterOpts,
    ): Int8Array;
    /**
     * Convert a value into a `Int16Array` (Int16Array).
     */
    Int16Array(
      v: any,
      prefix?: string,
      context?: string,
      opts?: BufferConverterOpts,
    ): Int16Array;
    /**
     * Convert a value into a `Int32Array` (Int32Array).
     */
    Int32Array(
      v: any,
      prefix?: string,
      context?: string,
      opts?: BufferConverterOpts,
    ): Int32Array;
    /**
     * Convert a value into a `Uint8Array` (Uint8Array).
     */
    Uint8Array(
      v: any,
      prefix?: string,
      context?: string,
      opts?: BufferConverterOpts,
    ): Uint8Array;
    /**
     * Convert a value into a `Uint16Array` (Uint16Array).
     */
    Uint16Array(
      v: any,
      prefix?: string,
      context?: string,
      opts?: BufferConverterOpts,
    ): Uint16Array;
    /**
     * Convert a value into a `Uint32Array` (Uint32Array).
     */
    Uint32Array(
      v: any,
      prefix?: string,
      context?: string,
      opts?: BufferConverterOpts,
    ): Uint32Array;
    /**
     * Convert a value into a `Uint8ClampedArray` (Uint8ClampedArray).
     */
    Uint8ClampedArray(
      v: any,
      prefix?: string,
      context?: string,
      opts?: BufferConverterOpts,
    ): Uint8ClampedArray;
    /**
     * Convert a value into a `Float32Array` (Float32Array).
     */
    Float32Array(
      v: any,
      prefix?: string,
      context?: string,
      opts?: BufferConverterOpts,
    ): Float32Array;
    /**
     * Convert a value into a `Float64Array` (Float64Array).
     */
    Float64Array(
      v: any,
      prefix?: string,
      context?: string,
      opts?: BufferConverterOpts,
    ): Float64Array;
    /**
     * Convert a value into an `ArrayBufferView` (ArrayBufferView).
     */
    ArrayBufferView(
      v: any,
      prefix?: string,
      context?: string,
      opts?: BufferConverterOpts,
    ): ArrayBufferView;
    /**
     * Convert a value into a `BufferSource` (ArrayBuffer or ArrayBufferView).
     */
    BufferSource(
      v: any,
      prefix?: string,
      context?: string,
      opts?: BufferConverterOpts,
    ): ArrayBuffer | ArrayBufferView;
    /**
     * Convert a value into a `DOMTimeStamp` (u64). Alias for unsigned long long
     */
    DOMTimeStamp(
      v: any,
      prefix?: string,
      context?: string,
      opts?: IntConverterOpts,
    ): number;
    /**
     * Convert a value into a `Function` ((...args: any[]) => any).
     */
    Function(
      v: any,
      prefix?: string,
      context?: string,
      opts?: any,
    ): (...args: any) => any;
    /**
     * Convert a value into a `VoidFunction` (() => void).
     */
    VoidFunction(
      v: any,
      prefix?: string,
      context?: string,
      opts?: any,
    ): () => void;
    ["UVString?"](
      v: any,
      prefix?: string,
      context?: string,
      opts?: StringConverterOpts,
    ): string | null;
    ["sequence<double>"](
      v: any,
      prefix?: string,
      context?: string,
      opts?: any,
    ): number[];

    [type: string]: (
      v: any,
      prefix?: string,
      context?: string,
      opts?: any,
    ) => any;
  };

  /**
   * Assert that the a function has at least a required amount of arguments.
   */
  function requiredArguments(
    length: number,
    required: number,
    prefix: string,
  ): void;
  type Dictionary = DictionaryMember[];
  interface DictionaryMember {
    key: string;
    converter: (v: any, prefix?: string, context?: string, opts?: any) => any;
    defaultValue?: any;
    required?: boolean;
  }

  /**
   * Create a converter for dictionaries.
   */
  function createDictionaryConverter<T>(
    name: string,
    ...dictionaries: Dictionary[]
  ): (v: any, prefix?: string, context?: string, opts?: any) => T;

  /**
   * Create a converter for enums.
   */
  function createEnumConverter(
    name: string,
    values: string[],
  ): (v: any, prefix?: string, context?: string, opts?: any) => string;

  /**
   * Create a converter that makes the contained type nullable.
   */
  function createNullableConverter<T>(
    converter: (v: any, prefix?: string, context?: string, opts?: any) => T,
  ): (v: any, prefix?: string, context?: string, opts?: any) => T | null;

  /**
   * Create a converter that converts a sequence of the inner type.
   */
  function createSequenceConverter<T>(
    converter: (v: any, prefix?: string, context?: string, opts?: any) => T,
  ): (v: any, prefix?: string, context?: string, opts?: any) => T[];

  /**
   * Create a converter that converts an async iterable of the inner type.
   */
  function createAsyncIterableConverter<V, T>(
    converter: (v: V, prefix?: string, context?: string, opts?: any) => T,
  ): (
    v: any,
    prefix?: string,
    context?: string,
    opts?: any,
  ) => ConvertedAsyncIterable<V, T>;

  interface ConvertedAsyncIterable<V, T> extends AsyncIterableIterator<T> {
    value: V;
  }

  /**
   * Create a converter that converts a Promise of the inner type.
   */
  function createPromiseConverter<T>(
    converter: (v: any, prefix?: string, context?: string, opts?: any) => T,
  ): (v: any, prefix?: string, context?: string, opts?: any) => Promise<T>;

  /**
   * Invoke a callback function.
   */
  function invokeCallbackFunction<T>(
    callable: (...args: any) => any,
    args: any[],
    thisArg: any,
    returnValueConverter: (
      v: any,
      prefix?: string,
      context?: string,
      opts?: any,
    ) => T,
    prefix: string,
    returnsPromise?: boolean,
  ): T;

  /**
   * Throw an illegal constructor error.
   */
  function illegalConstructor(): never;

  /**
   * The branding symbol.
   */
  const brand: unique symbol;

  /**
   * Create a branded instance of an interface.
   */
  function createBranded(self: any): any;

  /**
   * Assert that self is branded.
   */
  function assertBranded(self: any, type: any): void;

  /**
   * Create a converter for interfaces.
   */
  function createInterfaceConverter(
    name: string,
    prototype: any,
  ): (v: any, prefix?: string, context?: string, opts?: any) => any;

  function createRecordConverter<K extends string | number | symbol, V>(
    keyConverter: (v: any, prefix?: string, context?: string, opts?: any) => K,
    valueConverter: (
      v: any,
      prefix?: string,
      context?: string,
      opts?: any,
    ) => V,
  ): (v: Record<K, V>, prefix?: string, context?: string, opts?: any) => any;

  /**
   * Mix in the iterable declarations defined in WebIDL.
   * https://heycam.github.io/webidl/#es-iterable
   */
  function mixinPairIterable(
    name: string,
    prototype: any,
    dataSymbol: symbol,
    keyKey: string | number | symbol,
    valueKey: string | number | symbol,
  ): void;

  /**
   * Configure prototype properties enumerability / writability / configurability.
   */
  function configureInterface(prototype: any);

  /**
   * Get the WebIDL / ES type of a value.
   */
  function type(
    v: any,
  ):
    | "Null"
    | "Undefined"
    | "Boolean"
    | "Number"
    | "String"
    | "Symbol"
    | "BigInt"
    | "Object";

  /**
   * Check whether a value is an async iterable.
   */
  function isAsyncIterable(v: any): boolean;
}

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
  function pathFromURL(pathOrURL: string | URL): string;
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
  function extractMimeType(headerValues: string[] | null): MimeType | null;
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
  type Transferable =
    | {
      kind: "messagePort";
      data: number;
    }
    | {
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

declare module "ext:deno_web/00_url.js" {
  const URL: typeof globalThis.URL;
  const URLPrototype: typeof globalThis.URL.prototype;
  const URLSearchParams: typeof globalThis.URLSearchParams;
  function parseUrlEncoded(bytes: Uint8Array): [string, string][];
}

declare module "ext:deno_web/01_urlpattern.js" {
  const URLPattern: typeof URLPattern;
}

declare module "ext:deno_web/01_console.js" {
  function createFilteredInspectProxy<TObject>(params: {
    object: TObject;
    keys: (keyof TObject)[];
    evaluate: boolean;
  }): Record<string, unknown>;

  class Console {}
}
