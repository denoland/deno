// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
// deno-lint-ignore-file no-explicit-any

/// <reference no-default-lib="true" />
/// <reference lib="esnext" />

declare module "ext:deno_webidl/00_webidl.js" {
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
    float(
      v: any,
      prefix?: string,
      context?: string,
      opts?: any,
    ): number;
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
    double(
      v: any,
      prefix?: string,
      context?: string,
      opts?: any,
    ): number;
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
    object(
      v: any,
      prefix?: string,
      context?: string,
      opts?: any,
    ): object;
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
    converter: (
      v: any,
      prefix?: string,
      context?: string,
      opts?: any,
    ) => any;
    defaultValue?: any;
    required?: boolean;
  }

  /**
   * Create a converter for dictionaries.
   */
  function createDictionaryConverter<T>(
    name: string,
    ...dictionaries: Dictionary[]
  ): (
    v: any,
    prefix?: string,
    context?: string,
    opts?: any,
  ) => T;

  /**
   * Create a converter for enums.
   */
  function createEnumConverter(
    name: string,
    values: string[],
  ): (
    v: any,
    prefix?: string,
    context?: string,
    opts?: any,
  ) => string;

  /**
   * Create a converter that makes the contained type nullable.
   */
  function createNullableConverter<T>(
    converter: (
      v: any,
      prefix?: string,
      context?: string,
      opts?: any,
    ) => T,
  ): (
    v: any,
    prefix?: string,
    context?: string,
    opts?: any,
  ) => T | null;

  /**
   * Create a converter that converts a sequence of the inner type.
   */
  function createSequenceConverter<T>(
    converter: (
      v: any,
      prefix?: string,
      context?: string,
      opts?: any,
    ) => T,
  ): (
    v: any,
    prefix?: string,
    context?: string,
    opts?: any,
  ) => T[];

  /**
   * Create a converter that converts a Promise of the inner type.
   */
  function createPromiseConverter<T>(
    converter: (
      v: any,
      prefix?: string,
      context?: string,
      opts?: any,
    ) => T,
  ): (
    v: any,
    prefix?: string,
    context?: string,
    opts?: any,
  ) => Promise<T>;

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
  ): (
    v: any,
    prefix?: string,
    context?: string,
    opts?: any,
  ) => any;

  function createRecordConverter<
    K extends string | number | symbol,
    V,
  >(
    keyConverter: (
      v: any,
      prefix?: string,
      context?: string,
      opts?: any,
    ) => K,
    valueConverter: (
      v: any,
      prefix?: string,
      context?: string,
      opts?: any,
    ) => V,
  ): (
    v: Record<K, V>,
    prefix?: string,
    context?: string,
    opts?: any,
  ) => any;

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
}
