// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

/// <reference no-default-lib="true" />
/// <reference lib="deno.ns" />
/// <reference lib="deno.broadcast_channel" />

declare namespace Deno {
  export {}; // stop default export type behavior

  /** **UNSTABLE**: New API, yet to be vetted.
   *
   * Retrieve the process umask.  If `mask` is provided, sets the process umask.
   * This call always returns what the umask was before the call.
   *
   * ```ts
   * console.log(Deno.umask());  // e.g. 18 (0o022)
   * const prevUmaskValue = Deno.umask(0o077);  // e.g. 18 (0o022)
   * console.log(Deno.umask());  // e.g. 63 (0o077)
   * ```
   *
   * This API is under consideration to determine if permissions are required to
   * call it.
   *
   * *Note*: This API is not implemented on Windows
   *
   * @category File System
   */
  export function umask(mask?: number): number;

  /** **UNSTABLE**: New API, yet to be vetted.
   *
   * All plain number types for interfacing with foreign functions.
   *
   * @category FFI
   */
  type NativeNumberType =
    | "u8"
    | "i8"
    | "u16"
    | "i16"
    | "u32"
    | "i32"
    | "f32"
    | "f64";

  /** **UNSTABLE**: New API, yet to be vetted.
   *
   * All BigInt number types for interfacing with foreign functions.
   *
   * @category FFI
   */
  type NativeBigIntType =
    | "u64"
    | "i64"
    | "usize"
    | "isize";

  /** **UNSTABLE**: New API, yet to be vetted.
   *
   * The native boolean type for interfacing to foreign functions.
   *
   * @category FFI
   */
  type NativeBooleanType = "bool";

  /** **UNSTABLE**: New API, yet to be vetted.
   *
   * The native pointer type for interfacing to foreign functions.
   *
   * @category FFI
   */
  type NativePointerType = "pointer";

  /** **UNSTABLE**: New API, yet to be vetted.
   *
   * The native buffer type for interfacing to foreign functions.
   *
   * @category FFI
   */
  type NativeBufferType = "buffer";

  /** **UNSTABLE**: New API, yet to be vetted.
   *
   * The native function type for interfacing with foreign functions.
   *
   * @category FFI
   */
  type NativeFunctionType = "function";

  /** **UNSTABLE**: New API, yet to be vetted.
   *
   * The native void type for interfacing with foreign functions.
   *
   * @category FFI
   */
  type NativeVoidType = "void";

  /** **UNSTABLE**: New API, yet to be vetted.
   *
   * The native struct type for interfacing with foreign functions.
   *
   * @category FFI
   */
  type NativeStructType = { readonly struct: readonly NativeType[] };

  /** @category FFI */
  const brand: unique symbol;

  /** @category FFI */
  export type NativeU8Enum<T extends number> = "u8" & { [brand]: T };
  /** @category FFI */
  export type NativeI8Enum<T extends number> = "i8" & { [brand]: T };
  /** @category FFI */
  export type NativeU16Enum<T extends number> = "u16" & { [brand]: T };
  /** @category FFI */
  export type NativeI16Enum<T extends number> = "i16" & { [brand]: T };
  /** @category FFI */
  export type NativeU32Enum<T extends number> = "u32" & { [brand]: T };
  /** @category FFI */
  export type NativeI32Enum<T extends number> = "i32" & { [brand]: T };
  /** @category FFI */
  export type NativeTypedPointer<T extends PointerObject> = "pointer" & {
    [brand]: T;
  };
  export type NativeTypedFunction<T extends UnsafeCallbackDefinition> =
    & "function"
    & {
      [brand]: T;
    };

  /** **UNSTABLE**: New API, yet to be vetted.
   *
   * All supported types for interfacing with foreign functions.
   *
   * @category FFI
   */
  export type NativeType =
    | NativeNumberType
    | NativeBigIntType
    | NativeBooleanType
    | NativePointerType
    | NativeBufferType
    | NativeFunctionType
    | NativeStructType;

  /** **UNSTABLE**: New API, yet to be vetted.
   *
   * @category FFI
   */
  export type NativeResultType = NativeType | NativeVoidType;

  /** **UNSTABLE**: New API, yet to be vetted.
   *
   * Type conversion for foreign symbol parameters and unsafe callback return
   * types.
   *
   * @category FFI
   */
  type ToNativeType<T extends NativeType = NativeType> = T extends
    NativeStructType ? BufferSource
    : T extends NativeNumberType ? T extends NativeU8Enum<infer U> ? U
      : T extends NativeI8Enum<infer U> ? U
      : T extends NativeU16Enum<infer U> ? U
      : T extends NativeI16Enum<infer U> ? U
      : T extends NativeU32Enum<infer U> ? U
      : T extends NativeI32Enum<infer U> ? U
      : number
    : T extends NativeBigIntType ? number | bigint
    : T extends NativeBooleanType ? boolean
    : T extends NativePointerType
      ? T extends NativeTypedPointer<infer U> ? U | null : PointerValue
    : T extends NativeFunctionType
      ? T extends NativeTypedFunction<infer U> ? PointerValue<U> | null
      : PointerValue
    : T extends NativeBufferType ? BufferSource | null
    : never;

  /** **UNSTABLE**: New API, yet to be vetted.
   *
   * Type conversion for unsafe callback return types.
   *
   * @category FFI
   */
  type ToNativeResultType<T extends NativeResultType = NativeResultType> =
    T extends NativeStructType ? BufferSource
      : T extends NativeNumberType ? T extends NativeU8Enum<infer U> ? U
        : T extends NativeI8Enum<infer U> ? U
        : T extends NativeU16Enum<infer U> ? U
        : T extends NativeI16Enum<infer U> ? U
        : T extends NativeU32Enum<infer U> ? U
        : T extends NativeI32Enum<infer U> ? U
        : number
      : T extends NativeBigIntType ? number | bigint
      : T extends NativeBooleanType ? boolean
      : T extends NativePointerType
        ? T extends NativeTypedPointer<infer U> ? U | null : PointerValue
      : T extends NativeFunctionType
        ? T extends NativeTypedFunction<infer U> ? PointerObject<U> | null
        : PointerValue
      : T extends NativeBufferType ? BufferSource | null
      : T extends NativeVoidType ? void
      : never;

  /** **UNSTABLE**: New API, yet to be vetted.
   *
   * A utility type for conversion of parameter types of foreign functions.
   *
   * @category FFI
   */
  type ToNativeParameterTypes<T extends readonly NativeType[]> =
    //
    [(T[number])[]] extends [T] ? ToNativeType<T[number]>[]
      : [readonly (T[number])[]] extends [T]
        ? readonly ToNativeType<T[number]>[]
      : T extends readonly [...NativeType[]] ? {
          [K in keyof T]: ToNativeType<T[K]>;
        }
      : never;

  /** **UNSTABLE**: New API, yet to be vetted.
   *
   * Type conversion for foreign symbol return types and unsafe callback
   * parameters.
   *
   * @category FFI
   */
  type FromNativeType<T extends NativeType = NativeType> = T extends
    NativeStructType ? Uint8Array
    : T extends NativeNumberType ? T extends NativeU8Enum<infer U> ? U
      : T extends NativeI8Enum<infer U> ? U
      : T extends NativeU16Enum<infer U> ? U
      : T extends NativeI16Enum<infer U> ? U
      : T extends NativeU32Enum<infer U> ? U
      : T extends NativeI32Enum<infer U> ? U
      : number
    : T extends NativeBigIntType ? number | bigint
    : T extends NativeBooleanType ? boolean
    : T extends NativePointerType
      ? T extends NativeTypedPointer<infer U> ? U | null : PointerValue
    : T extends NativeBufferType ? PointerValue
    : T extends NativeFunctionType
      ? T extends NativeTypedFunction<infer U> ? PointerObject<U> | null
      : PointerValue
    : never;

  /** **UNSTABLE**: New API, yet to be vetted.
   *
   * Type conversion for foreign symbol return types.
   *
   * @category FFI
   */
  type FromNativeResultType<T extends NativeResultType = NativeResultType> =
    T extends NativeStructType ? Uint8Array
      : T extends NativeNumberType ? T extends NativeU8Enum<infer U> ? U
        : T extends NativeI8Enum<infer U> ? U
        : T extends NativeU16Enum<infer U> ? U
        : T extends NativeI16Enum<infer U> ? U
        : T extends NativeU32Enum<infer U> ? U
        : T extends NativeI32Enum<infer U> ? U
        : number
      : T extends NativeBigIntType ? number | bigint
      : T extends NativeBooleanType ? boolean
      : T extends NativePointerType
        ? T extends NativeTypedPointer<infer U> ? U | null : PointerValue
      : T extends NativeBufferType ? PointerValue
      : T extends NativeFunctionType
        ? T extends NativeTypedFunction<infer U> ? PointerObject<U> | null
        : PointerValue
      : T extends NativeVoidType ? void
      : never;

  /** **UNSTABLE**: New API, yet to be vetted.
   *
   * @category FFI
   */
  type FromNativeParameterTypes<
    T extends readonly NativeType[],
  > =
    //
    [(T[number])[]] extends [T] ? FromNativeType<T[number]>[]
      : [readonly (T[number])[]] extends [T]
        ? readonly FromNativeType<T[number]>[]
      : T extends readonly [...NativeType[]] ? {
          [K in keyof T]: FromNativeType<T[K]>;
        }
      : never;

  /** **UNSTABLE**: New API, yet to be vetted.
   *
   * The interface for a foreign function as defined by its parameter and result
   * types.
   *
   * @category FFI
   */
  export interface ForeignFunction<
    Parameters extends readonly NativeType[] = readonly NativeType[],
    Result extends NativeResultType = NativeResultType,
    NonBlocking extends boolean = boolean,
  > {
    /** Name of the symbol.
     *
     * Defaults to the key name in symbols object. */
    name?: string;
    /** The parameters of the foreign function. */
    parameters: Parameters;
    /** The result (return value) of the foreign function. */
    result: Result;
    /** When `true`, function calls will run on a dedicated blocking thread and
     * will return a `Promise` resolving to the `result`. */
    nonblocking?: NonBlocking;
    /** When `true`, function calls can safely callback into JavaScript or
     * trigger a garbage collection event.
     *
     * @default {false} */
    callback?: boolean;
    /** When `true`, dlopen will not fail if the symbol is not found.
     * Instead, the symbol will be set to `null`.
     *
     * @default {false} */
    optional?: boolean;
  }

  /** **UNSTABLE**: New API, yet to be vetted.
   *
   * @category FFI
   */
  export interface ForeignStatic<Type extends NativeType = NativeType> {
    /** Name of the symbol, defaults to the key name in symbols object. */
    name?: string;
    /** The type of the foreign static value. */
    type: Type;
    /** When `true`, dlopen will not fail if the symbol is not found.
     * Instead, the symbol will be set to `null`.
     *
     * @default {false} */
    optional?: boolean;
  }

  /** **UNSTABLE**: New API, yet to be vetted.
   *
   * A foreign library interface descriptor.
   *
   * @category FFI
   */
  export interface ForeignLibraryInterface {
    [name: string]: ForeignFunction | ForeignStatic;
  }

  /** **UNSTABLE**: New API, yet to be vetted.
   *
   * A utility type that infers a foreign symbol.
   *
   * @category FFI
   */
  type StaticForeignSymbol<T extends ForeignFunction | ForeignStatic> =
    T extends ForeignFunction ? FromForeignFunction<T>
      : T extends ForeignStatic ? FromNativeType<T["type"]>
      : never;

  /** **UNSTABLE**: New API, yet to be vetted.
   *
   *  @category FFI
   */
  type FromForeignFunction<T extends ForeignFunction> = T["parameters"] extends
    readonly [] ? () => StaticForeignSymbolReturnType<T>
    : (
      ...args: ToNativeParameterTypes<T["parameters"]>
    ) => StaticForeignSymbolReturnType<T>;

  /** **UNSTABLE**: New API, yet to be vetted.
   *
   * @category FFI
   */
  type StaticForeignSymbolReturnType<T extends ForeignFunction> =
    ConditionalAsync<T["nonblocking"], FromNativeResultType<T["result"]>>;

  /** **UNSTABLE**: New API, yet to be vetted.
   *
   * @category FFI
   */
  type ConditionalAsync<IsAsync extends boolean | undefined, T> =
    IsAsync extends true ? Promise<T> : T;

  /** **UNSTABLE**: New API, yet to be vetted.
   *
   * A utility type that infers a foreign library interface.
   *
   * @category FFI
   */
  type StaticForeignLibraryInterface<T extends ForeignLibraryInterface> = {
    [K in keyof T]: T[K]["optional"] extends true
      ? StaticForeignSymbol<T[K]> | null
      : StaticForeignSymbol<T[K]>;
  };

  /** **UNSTABLE**: New API, yet to be vetted.
   *
   * A non-null pointer, represented as an object
   * at runtime. The object's prototype is `null`
   * and cannot be changed. The object cannot be
   * assigned to either and is thus entirely read-only.
   *
   * To interact with memory through a pointer use the
   * {@linkcode UnsafePointerView} class. To create a
   * pointer from an address or the get the address of
   * a pointer use the static methods of the
   * {@linkcode UnsafePointer} class.
   *
   * @category FFI
   */
  export type PointerObject<T = unknown> = { [brand]: T };

  /** **UNSTABLE**: New API, yet to be vetted.
   *
   * Pointers are represented either with a {@linkcode PointerObject}
   * object or a `null` if the pointer is null.
   *
   * @category FFI
   */
  export type PointerValue<T = unknown> = null | PointerObject<T>;

  /** **UNSTABLE**: New API, yet to be vetted.
   *
   * A collection of static functions for interacting with pointer objects.
   *
   * @category FFI
   */
  export class UnsafePointer {
    /** Create a pointer from a numeric value. This one is <i>really</i> dangerous! */
    static create<T = unknown>(value: number | bigint): PointerValue<T>;
    /** Returns `true` if the two pointers point to the same address. */
    static equals<T = unknown>(a: PointerValue<T>, b: PointerValue<T>): boolean;
    /** Return the direct memory pointer to the typed array in memory. */
    static of<T = unknown>(
      value: Deno.UnsafeCallback | BufferSource,
    ): PointerValue<T>;
    /** Return a new pointer offset from the original by `offset` bytes. */
    static offset<T = unknown>(
      value: PointerObject,
      offset: number,
    ): PointerValue<T>;
    /** Get the numeric value of a pointer */
    static value(value: PointerValue): number | bigint;
  }

  /** **UNSTABLE**: New API, yet to be vetted.
   *
   * An unsafe pointer view to a memory location as specified by the `pointer`
   * value. The `UnsafePointerView` API follows the standard built in interface
   * {@linkcode DataView} for accessing the underlying types at an memory
   * location (numbers, strings and raw bytes).
   *
   * @category FFI
   */
  export class UnsafePointerView {
    constructor(pointer: PointerObject);

    pointer: PointerObject;

    /** Gets a boolean at the specified byte offset from the pointer. */
    getBool(offset?: number): boolean;
    /** Gets an unsigned 8-bit integer at the specified byte offset from the
     * pointer. */
    getUint8(offset?: number): number;
    /** Gets a signed 8-bit integer at the specified byte offset from the
     * pointer. */
    getInt8(offset?: number): number;
    /** Gets an unsigned 16-bit integer at the specified byte offset from the
     * pointer. */
    getUint16(offset?: number): number;
    /** Gets a signed 16-bit integer at the specified byte offset from the
     * pointer. */
    getInt16(offset?: number): number;
    /** Gets an unsigned 32-bit integer at the specified byte offset from the
     * pointer. */
    getUint32(offset?: number): number;
    /** Gets a signed 32-bit integer at the specified byte offset from the
     * pointer. */
    getInt32(offset?: number): number;
    /** Gets an unsigned 64-bit integer at the specified byte offset from the
     * pointer. */
    getBigUint64(offset?: number): number | bigint;
    /** Gets a signed 64-bit integer at the specified byte offset from the
     * pointer. */
    getBigInt64(offset?: number): number | bigint;
    /** Gets a signed 32-bit float at the specified byte offset from the
     * pointer. */
    getFloat32(offset?: number): number;
    /** Gets a signed 64-bit float at the specified byte offset from the
     * pointer. */
    getFloat64(offset?: number): number;
    /** Gets a pointer at the specified byte offset from the pointer */
    getPointer<T = unknown>(offset?: number): PointerValue<T>;
    /** Gets a C string (`null` terminated string) at the specified byte offset
     * from the pointer. */
    getCString(offset?: number): string;
    /** Gets a C string (`null` terminated string) at the specified byte offset
     * from the specified pointer. */
    static getCString(
      pointer: PointerObject,
      offset?: number,
    ): string;
    /** Gets an `ArrayBuffer` of length `byteLength` at the specified byte
     * offset from the pointer. */
    getArrayBuffer(byteLength: number, offset?: number): ArrayBuffer;
    /** Gets an `ArrayBuffer` of length `byteLength` at the specified byte
     * offset from the specified pointer. */
    static getArrayBuffer(
      pointer: PointerObject,
      byteLength: number,
      offset?: number,
    ): ArrayBuffer;
    /** Copies the memory of the pointer into a typed array.
     *
     * Length is determined from the typed array's `byteLength`.
     *
     * Also takes optional byte offset from the pointer. */
    copyInto(destination: BufferSource, offset?: number): void;
    /** Copies the memory of the specified pointer into a typed array.
     *
     * Length is determined from the typed array's `byteLength`.
     *
     * Also takes optional byte offset from the pointer. */
    static copyInto(
      pointer: PointerObject,
      destination: BufferSource,
      offset?: number,
    ): void;
  }

  /** **UNSTABLE**: New API, yet to be vetted.
   *
   * An unsafe pointer to a function, for calling functions that are not present
   * as symbols.
   *
   * @category FFI
   */
  export class UnsafeFnPointer<Fn extends ForeignFunction> {
    /** The pointer to the function. */
    pointer: PointerObject<Fn>;
    /** The definition of the function. */
    definition: Fn;

    constructor(pointer: PointerObject<Fn>, definition: Const<Fn>);
    /** @deprecated Properly type {@linkcode pointer} using {@linkcode NativeTypedFunction} or {@linkcode UnsafeCallbackDefinition} types. */
    constructor(pointer: PointerObject, definition: Const<Fn>);

    /** Call the foreign function. */
    call: FromForeignFunction<Fn>;
  }

  /** **UNSTABLE**: New API, yet to be vetted.
   *
   * Definition of a unsafe callback function.
   *
   * @category FFI
   */
  export interface UnsafeCallbackDefinition<
    Parameters extends readonly NativeType[] = readonly NativeType[],
    Result extends NativeResultType = NativeResultType,
  > {
    /** The parameters of the callbacks. */
    parameters: Parameters;
    /** The current result of the callback. */
    result: Result;
  }

  /** **UNSTABLE**: New API, yet to be vetted.
   *
   * An unsafe callback function.
   *
   * @category FFI
   */
  type UnsafeCallbackFunction<
    Parameters extends readonly NativeType[] = readonly NativeType[],
    Result extends NativeResultType = NativeResultType,
  > = Parameters extends readonly [] ? () => ToNativeResultType<Result> : (
    ...args: FromNativeParameterTypes<Parameters>
  ) => ToNativeResultType<Result>;

  /** **UNSTABLE**: New API, yet to be vetted.
   *
   * An unsafe function pointer for passing JavaScript functions as C function
   * pointers to foreign function calls.
   *
   * The function pointer remains valid until the `close()` method is called.
   *
   * All `UnsafeCallback` are always thread safe in that they can be called from
   * foreign threads without crashing. However, they do not wake up the Deno event
   * loop by default.
   *
   * If a callback is to be called from foreign threads, use the `threadSafe()`
   * static constructor or explicitly call `ref()` to have the callback wake up
   * the Deno event loop when called from foreign threads. This also stops
   * Deno's process from exiting while the callback still exists and is not
   * unref'ed.
   *
   * Use `deref()` to then allow Deno's process to exit. Calling `deref()` on
   * a ref'ed callback does not stop it from waking up the Deno event loop when
   * called from foreign threads.
   *
   * @category FFI
   */
  export class UnsafeCallback<
    Definition extends UnsafeCallbackDefinition = UnsafeCallbackDefinition,
  > {
    constructor(
      definition: Const<Definition>,
      callback: UnsafeCallbackFunction<
        Definition["parameters"],
        Definition["result"]
      >,
    );

    /** The pointer to the unsafe callback. */
    readonly pointer: PointerObject<Definition>;
    /** The definition of the unsafe callback. */
    readonly definition: Definition;
    /** The callback function. */
    readonly callback: UnsafeCallbackFunction<
      Definition["parameters"],
      Definition["result"]
    >;

    /**
     * Creates an {@linkcode UnsafeCallback} and calls `ref()` once to allow it to
     * wake up the Deno event loop when called from foreign threads.
     *
     * This also stops Deno's process from exiting while the callback still
     * exists and is not unref'ed.
     */
    static threadSafe<
      Definition extends UnsafeCallbackDefinition = UnsafeCallbackDefinition,
    >(
      definition: Const<Definition>,
      callback: UnsafeCallbackFunction<
        Definition["parameters"],
        Definition["result"]
      >,
    ): UnsafeCallback<Definition>;

    /**
     * Increments the callback's reference counting and returns the new
     * reference count.
     *
     * After `ref()` has been called, the callback always wakes up the
     * Deno event loop when called from foreign threads.
     *
     * If the callback's reference count is non-zero, it keeps Deno's
     * process from exiting.
     */
    ref(): number;

    /**
     * Decrements the callback's reference counting and returns the new
     * reference count.
     *
     * Calling `unref()` does not stop a callback from waking up the Deno
     * event loop when called from foreign threads.
     *
     * If the callback's reference counter is zero, it no longer keeps
     * Deno's process from exiting.
     */
    unref(): number;

    /**
     * Removes the C function pointer associated with this instance.
     *
     * Continuing to use the instance or the C function pointer after closing
     * the `UnsafeCallback` will lead to errors and crashes.
     *
     * Calling this method sets the callback's reference counting to zero,
     * stops the callback from waking up the Deno event loop when called from
     * foreign threads and no longer keeps Deno's process from exiting.
     */
    close(): void;
  }

  /** **UNSTABLE**: New API, yet to be vetted.
   *
   * A dynamic library resource.  Use {@linkcode Deno.dlopen} to load a dynamic
   * library and return this interface.
   *
   * @category FFI
   */
  export interface DynamicLibrary<S extends ForeignLibraryInterface> {
    /** All of the registered library along with functions for calling them. */
    symbols: StaticForeignLibraryInterface<S>;
    /** Removes the pointers associated with the library symbols.
     *
     * Continuing to use symbols that are part of the library will lead to
     * errors and crashes.
     *
     * Calling this method will also immediately set any references to zero and
     * will no longer keep Deno's process from exiting.
     */
    close(): void;
  }

  /**
   *  This magic code used to implement better type hints for {@linkcode Deno.dlopen}
   *
   *  @category FFI
   */
  type Cast<A, B> = A extends B ? A : B;
  /** @category FFI */
  type Const<T> = Cast<
    T,
    | (T extends string | number | bigint | boolean ? T : never)
    | { [K in keyof T]: Const<T[K]> }
    | []
  >;

  /** **UNSTABLE**: New API, yet to be vetted.
   *
   * Opens an external dynamic library and registers symbols, making foreign
   * functions available to be called.
   *
   * Requires `allow-ffi` permission. Loading foreign dynamic libraries can in
   * theory bypass all of the sandbox permissions. While it is a separate
   * permission users should acknowledge in practice that is effectively the
   * same as running with the `allow-all` permission.
   *
   * @example Given a C library which exports a foreign function named `add()`
   *
   * ```ts
   * // Determine library extension based on
   * // your OS.
   * let libSuffix = "";
   * switch (Deno.build.os) {
   *   case "windows":
   *     libSuffix = "dll";
   *     break;
   *   case "darwin":
   *     libSuffix = "dylib";
   *     break;
   *   default:
   *     libSuffix = "so";
   *     break;
   * }
   *
   * const libName = `./libadd.${libSuffix}`;
   * // Open library and define exported symbols
   * const dylib = Deno.dlopen(
   *   libName,
   *   {
   *     "add": { parameters: ["isize", "isize"], result: "isize" },
   *   } as const,
   * );
   *
   * // Call the symbol `add`
   * const result = dylib.symbols.add(35, 34); // 69
   *
   * console.log(`Result from external addition of 35 and 34: ${result}`);
   * ```
   *
   * @tags allow-ffi
   * @category FFI
   */
  export function dlopen<S extends ForeignLibraryInterface>(
    filename: string | URL,
    symbols: Const<S>,
  ): DynamicLibrary<S>;

  /** **UNSTABLE**: New API, yet to be vetted.
   *
   * These are unstable options which can be used with {@linkcode Deno.run}.
   *
   * @category Sub Process
   */
  interface UnstableRunOptions extends RunOptions {
    /** If `true`, clears the environment variables before executing the
     * sub-process.
     *
     * @default {false} */
    clearEnv?: boolean;
    /** For POSIX systems, sets the group ID for the sub process. */
    gid?: number;
    /** For POSIX systems, sets the user ID for the sub process. */
    uid?: number;
  }

  /** **UNSTABLE**: New API, yet to be vetted.
   *
   * Spawns new subprocess. RunOptions must contain at a minimum the `opt.cmd`,
   * an array of program arguments, the first of which is the binary.
   *
   * ```ts
   * const p = Deno.run({
   *   cmd: ["curl", "https://example.com"],
   * });
   * const status = await p.status();
   * ```
   *
   * Subprocess uses same working directory as parent process unless `opt.cwd`
   * is specified.
   *
   * Environmental variables from parent process can be cleared using `opt.clearEnv`.
   * Doesn't guarantee that only `opt.env` variables are present,
   * as the OS may set environmental variables for processes.
   *
   * Environmental variables for subprocess can be specified using `opt.env`
   * mapping.
   *
   * `opt.uid` sets the child process’s user ID. This translates to a setuid call
   * in the child process. Failure in the setuid call will cause the spawn to fail.
   *
   * `opt.gid` is similar to `opt.uid`, but sets the group ID of the child process.
   * This has the same semantics as the uid field.
   *
   * By default subprocess inherits stdio of parent process. To change
   * this this, `opt.stdin`, `opt.stdout`, and `opt.stderr` can be set
   * independently to a resource ID (_rid_) of an open file, `"inherit"`,
   * `"piped"`, or `"null"`:
   *
   * - _number_: the resource ID of an open file/resource. This allows you to
   *   read or write to a file.
   * - `"inherit"`: The default if unspecified. The subprocess inherits from the
   *   parent.
   * - `"piped"`: A new pipe should be arranged to connect the parent and child
   *   sub-process.
   * - `"null"`: This stream will be ignored. This is the equivalent of attaching
   *   the stream to `/dev/null`.
   *
   * Details of the spawned process are returned as an instance of
   * {@linkcode Deno.Process}.
   *
   * Requires `allow-run` permission.
   *
   * @tags allow-run
   * @category Sub Process
   */
  export function run<T extends UnstableRunOptions = UnstableRunOptions>(
    opt: T,
  ): Process<T>;

  /** **UNSTABLE**: New API, yet to be vetted.
   *
   * A custom `HttpClient` for use with {@linkcode fetch} function. This is
   * designed to allow custom certificates or proxies to be used with `fetch()`.
   *
   * @example ```ts
   * const caCert = await Deno.readTextFile("./ca.pem");
   * const client = Deno.createHttpClient({ caCerts: [ caCert ] });
   * const req = await fetch("https://myserver.com", { client });
   * ```
   *
   * @category Fetch API
   */
  export interface HttpClient extends Disposable {
    /** The resource ID associated with the client. */
    rid: number;
    /** Close the HTTP client. */
    close(): void;
  }

  /** **UNSTABLE**: New API, yet to be vetted.
   *
   * The options used when creating a {@linkcode Deno.HttpClient}.
   *
   * @category Fetch API
   */
  export interface CreateHttpClientOptions {
    /** A list of root certificates that will be used in addition to the
     * default root certificates to verify the peer's certificate.
     *
     * Must be in PEM format. */
    caCerts?: string[];
    /** A HTTP proxy to use for new connections. */
    proxy?: Proxy;
    /** PEM formatted client certificate chain. */
    certChain?: string;
    /** PEM formatted (RSA or PKCS8) private key of client certificate. */
    privateKey?: string;
    /** Sets the maximum numer of idle connections per host allowed in the pool. */
    poolMaxIdlePerHost?: number;
    /** Set an optional timeout for idle sockets being kept-alive.
     * Set to false to disable the timeout. */
    poolIdleTimeout?: number | false;
    /**
     * Whether HTTP/1.1 is allowed or not.
     *
     * @default {true}
     */
    http1?: boolean;
    /** Whether HTTP/2 is allowed or not.
     *
     * @default {true}
     */
    http2?: boolean;
    /** Whether setting the host header is allowed or not.
     *
     * @default {false}
     */
    allowHost?: boolean;
  }

  /** **UNSTABLE**: New API, yet to be vetted.
   *
   * The definition of a proxy when specifying
   * {@linkcode Deno.CreateHttpClientOptions}.
   *
   * @category Fetch API
   */
  export interface Proxy {
    /** The string URL of the proxy server to use. */
    url: string;
    /** The basic auth credentials to be used against the proxy server. */
    basicAuth?: BasicAuth;
  }

  /** **UNSTABLE**: New API, yet to be vetted.
   *
   * Basic authentication credentials to be used with a {@linkcode Deno.Proxy}
   * server when specifying {@linkcode Deno.CreateHttpClientOptions}.
   *
   * @category Fetch API
   */
  export interface BasicAuth {
    /** The username to be used against the proxy server. */
    username: string;
    /** The password to be used against the proxy server. */
    password: string;
  }

  /** **UNSTABLE**: New API, yet to be vetted.
   *
   * Create a custom HttpClient to use with {@linkcode fetch}. This is an
   * extension of the web platform Fetch API which allows Deno to use custom
   * TLS certificates and connect via a proxy while using `fetch()`.
   *
   * @example ```ts
   * const caCert = await Deno.readTextFile("./ca.pem");
   * const client = Deno.createHttpClient({ caCerts: [ caCert ] });
   * const response = await fetch("https://myserver.com", { client });
   * ```
   *
   * @example ```ts
   * const client = Deno.createHttpClient({
   *   proxy: { url: "http://myproxy.com:8080" }
   * });
   * const response = await fetch("https://myserver.com", { client });
   * ```
   *
   * @category Fetch API
   */
  export function createHttpClient(
    options: CreateHttpClientOptions,
  ): HttpClient;

  /** **UNSTABLE**: New API, yet to be vetted.
   *
   * Represents membership of a IPv4 multicast group.
   *
   * @category Network
   */
  interface MulticastV4Membership {
    /** Leaves the multicast group. */
    leave: () => Promise<void>;
    /** Sets the multicast loopback option. If enabled, multicast packets will be looped back to the local socket. */
    setLoopback: (loopback: boolean) => Promise<void>;
    /** Sets the time-to-live of outgoing multicast packets for this socket. */
    setTTL: (ttl: number) => Promise<void>;
  }

  /** **UNSTABLE**: New API, yet to be vetted.
   *
   * Represents membership of a IPv6 multicast group.
   *
   * @category Network
   */
  interface MulticastV6Membership {
    /** Leaves the multicast group. */
    leave: () => Promise<void>;
    /** Sets the multicast loopback option. If enabled, multicast packets will be looped back to the local socket. */
    setLoopback: (loopback: boolean) => Promise<void>;
  }

  /** **UNSTABLE**: New API, yet to be vetted.
   *
   * A generic transport listener for message-oriented protocols.
   *
   * @category Network
   */
  export interface DatagramConn extends AsyncIterable<[Uint8Array, Addr]> {
    /** Joins an IPv4 multicast group. */
    joinMulticastV4(
      address: string,
      networkInterface: string,
    ): Promise<MulticastV4Membership>;

    /** Joins an IPv6 multicast group. */
    joinMulticastV6(
      address: string,
      networkInterface: number,
    ): Promise<MulticastV6Membership>;

    /** Waits for and resolves to the next message to the instance.
     *
     * Messages are received in the format of a tuple containing the data array
     * and the address information.
     */
    receive(p?: Uint8Array): Promise<[Uint8Array, Addr]>;
    /** Sends a message to the target via the connection. The method resolves
     * with the number of bytes sent. */
    send(p: Uint8Array, addr: Addr): Promise<number>;
    /** Close closes the socket. Any pending message promises will be rejected
     * with errors. */
    close(): void;
    /** Return the address of the instance. */
    readonly addr: Addr;
    [Symbol.asyncIterator](): AsyncIterableIterator<[Uint8Array, Addr]>;
  }

  /**
   * @category Network
   */
  export interface TcpListenOptions extends ListenOptions {
    /** When `true` the SO_REUSEPORT flag will be set on the listener. This
     * allows multiple processes to listen on the same address and port.
     *
     * On Linux this will cause the kernel to distribute incoming connections
     * across the different processes that are listening on the same address and
     * port.
     *
     * This flag is only supported on Linux. It is silently ignored on other
     * platforms.
     *
     * @default {false} */
    reusePort?: boolean;
  }

  /** **UNSTABLE**: New API, yet to be vetted.
   *
   * Unstable options which can be set when opening a Unix listener via
   * {@linkcode Deno.listen} or {@linkcode Deno.listenDatagram}.
   *
   * @category Network
   */
  export interface UnixListenOptions {
    /** A path to the Unix Socket. */
    path: string;
  }

  /** **UNSTABLE**: New API, yet to be vetted.
   *
   * Unstable options which can be set when opening a datagram listener via
   * {@linkcode Deno.listenDatagram}.
   *
   * @category Network
   */
  export interface UdpListenOptions extends ListenOptions {
    /** When `true` the specified address will be reused, even if another
     * process has already bound a socket on it. This effectively steals the
     * socket from the listener.
     *
     * @default {false} */
    reuseAddress?: boolean;

    /** When `true`, sent multicast packets will be looped back to the local socket.
     *
     * @default {false} */
    loopback?: boolean;
  }

  /** **UNSTABLE**: New API, yet to be vetted.
   *
   * Listen announces on the local transport address.
   *
   * ```ts
   * const listener = Deno.listen({ path: "/foo/bar.sock", transport: "unix" })
   * ```
   *
   * Requires `allow-read` and `allow-write` permission.
   *
   * @tags allow-read, allow-write
   * @category Network
   */
  export function listen(
    options: UnixListenOptions & { transport: "unix" },
  ): Listener;

  /** **UNSTABLE**: New API, yet to be vetted.
   *
   * Listen announces on the local transport address.
   *
   * ```ts
   * const listener1 = Deno.listenDatagram({
   *   port: 80,
   *   transport: "udp"
   * });
   * const listener2 = Deno.listenDatagram({
   *   hostname: "golang.org",
   *   port: 80,
   *   transport: "udp"
   * });
   * ```
   *
   * Requires `allow-net` permission.
   *
   * @tags allow-net
   * @category Network
   */
  export function listenDatagram(
    options: UdpListenOptions & { transport: "udp" },
  ): DatagramConn;

  /** **UNSTABLE**: New API, yet to be vetted.
   *
   * Listen announces on the local transport address.
   *
   * ```ts
   * const listener = Deno.listenDatagram({
   *   path: "/foo/bar.sock",
   *   transport: "unixpacket"
   * });
   * ```
   *
   * Requires `allow-read` and `allow-write` permission.
   *
   * @tags allow-read, allow-write
   * @category Network
   */
  export function listenDatagram(
    options: UnixListenOptions & { transport: "unixpacket" },
  ): DatagramConn;

  /** **UNSTABLE**: New API, yet to be vetted.
   *
   * @category Network
   */
  export interface UnixConnectOptions {
    transport: "unix";
    path: string;
  }

  /** **UNSTABLE**: New API, yet to be vetted.
   *
   * Connects to the hostname (default is "127.0.0.1") and port on the named
   * transport (default is "tcp"), and resolves to the connection (`Conn`).
   *
   * ```ts
   * const conn1 = await Deno.connect({ port: 80 });
   * const conn2 = await Deno.connect({ hostname: "192.0.2.1", port: 80 });
   * const conn3 = await Deno.connect({ hostname: "[2001:db8::1]", port: 80 });
   * const conn4 = await Deno.connect({ hostname: "golang.org", port: 80, transport: "tcp" });
   * const conn5 = await Deno.connect({ path: "/foo/bar.sock", transport: "unix" });
   * ```
   *
   * Requires `allow-net` permission for "tcp" and `allow-read` for "unix".
   *
   * @tags allow-net, allow-read
   * @category Network
   */
  export function connect(options: ConnectOptions): Promise<TcpConn>;
  /** **UNSTABLE**: New API, yet to be vetted.
   *
   * Connects to the hostname (default is "127.0.0.1") and port on the named
   * transport (default is "tcp"), and resolves to the connection (`Conn`).
   *
   * ```ts
   * const conn1 = await Deno.connect({ port: 80 });
   * const conn2 = await Deno.connect({ hostname: "192.0.2.1", port: 80 });
   * const conn3 = await Deno.connect({ hostname: "[2001:db8::1]", port: 80 });
   * const conn4 = await Deno.connect({ hostname: "golang.org", port: 80, transport: "tcp" });
   * const conn5 = await Deno.connect({ path: "/foo/bar.sock", transport: "unix" });
   * ```
   *
   * Requires `allow-net` permission for "tcp" and `allow-read` for "unix".
   *
   * @tags allow-net, allow-read
   * @category Network
   */
  export function connect(options: UnixConnectOptions): Promise<UnixConn>;

  /** **UNSTABLE**: New API, yet to be vetted.
   *
   * @category Network
   */
  export interface ConnectTlsOptions {
    /** **UNSTABLE**: New API, yet to be vetted.
     *
     * PEM formatted client certificate chain.
     */
    certChain?: string;
    /** **UNSTABLE**: New API, yet to be vetted.
     *
     * PEM formatted (RSA or PKCS8) private key of client certificate.
     */
    privateKey?: string;
  }

  /** **UNSTABLE**: New API, yet to be vetted.
   *
   * @category Network
   */
  export interface TlsHandshakeInfo {
    /** **UNSTABLE**: New API, yet to be vetted.
     *
     * Contains the ALPN protocol selected during negotiation with the server.
     * If no ALPN protocol selected, returns `null`.
     */
    alpnProtocol: string | null;
  }

  /** **UNSTABLE**: New API, yet to be vetted.
   *
   * @category Network
   */
  export interface TlsConn extends Conn {
    /** **UNSTABLE**: New API, yet to be vetted.
     *
     * Runs the client or server handshake protocol to completion if that has
     * not happened yet. Calling this method is optional; the TLS handshake
     * will be completed automatically as soon as data is sent or received.
     */
    handshake(): Promise<TlsHandshakeInfo>;
  }

  /** **UNSTABLE**: New API, yet to be vetted.
   *
   * Create a TLS connection with an attached client certificate.
   *
   * ```ts
   * const conn = await Deno.connectTls({
   *   hostname: "deno.land",
   *   port: 443,
   *   certChain: "---- BEGIN CERTIFICATE ----\n ...",
   *   privateKey: "---- BEGIN PRIVATE KEY ----\n ...",
   * });
   * ```
   *
   * Requires `allow-net` permission.
   *
   * @tags allow-net
   * @category Network
   */
  export function connectTls(options: ConnectTlsOptions): Promise<TlsConn>;

  /** **UNSTABLE**: New API, yet to be vetted.
   *
   * Acquire an advisory file-system lock for the provided file.
   *
   * @param [exclusive=false]
   * @category File System
   */
  export function flock(rid: number, exclusive?: boolean): Promise<void>;

  /** **UNSTABLE**: New API, yet to be vetted.
   *
   * Acquire an advisory file-system lock synchronously for the provided file.
   *
   * @param [exclusive=false]
   * @category File System
   */
  export function flockSync(rid: number, exclusive?: boolean): void;

  /** **UNSTABLE**: New API, yet to be vetted.
   *
   * Release an advisory file-system lock for the provided file.
   *
   * @category File System
   */
  export function funlock(rid: number): Promise<void>;

  /** **UNSTABLE**: New API, yet to be vetted.
   *
   * Release an advisory file-system lock for the provided file synchronously.
   *
   * @category File System
   */
  export function funlockSync(rid: number): void;

  /** **UNSTABLE**: New API, yet to be vetted.
   *
   * Allows "hijacking" the connection that the request is associated with. This
   * can be used to implement protocols that build on top of HTTP (eg.
   * {@linkcode WebSocket}).
   *
   * The returned promise returns underlying connection and first packet
   * received. The promise shouldn't be awaited before responding to the
   * `request`, otherwise event loop might deadlock.
   *
   * ```ts
   * function handler(req: Request): Response {
   *   Deno.upgradeHttp(req).then(([conn, firstPacket]) => {
   *     // ...
   *   });
   *   return new Response(null, { status: 101 });
   * }
   * ```
   *
   * This method can only be called on requests originating the
   * {@linkcode Deno.serveHttp} server.
   *
   * @category HTTP Server
   */
  export function upgradeHttp(
    request: Request,
  ): Promise<[Deno.Conn, Uint8Array]>;

  /** **UNSTABLE**: New API, yet to be vetted.
   *
   * Open a new {@linkcode Deno.Kv} connection to persist data.
   *
   * When a path is provided, the database will be persisted to disk at that
   * path. Read and write access to the file is required.
   *
   * When no path is provided, the database will be opened in a default path for
   * the current script. This location is persistent across script runs and is
   * keyed on the origin storage key (the same key that is used to determine
   * `localStorage` persistence). More information about the origin storage key
   * can be found in the Deno Manual.
   *
   * @tags allow-read, allow-write
   * @category KV
   */
  export function openKv(path?: string): Promise<Deno.Kv>;

  /** **UNSTABLE**: New API, yet to be vetted.
   *
   * CronScheduleExpression is used as the type of `minute`, `hour`,
   * `dayOfMonth`, `month`, and `dayOfWeek` in {@linkcode CronSchedule}.
   * @category Cron
   */
  type CronScheduleExpression = number | { exact: number | number[] } | {
    start?: number;
    end?: number;
    every?: number;
  };

  /** **UNSTABLE**: New API, yet to be vetted.
   *
   * CronSchedule is the interface used for JSON format
   * cron `schedule`.
   * @category Cron
   */
  export interface CronSchedule {
    minute?: CronScheduleExpression;
    hour?: CronScheduleExpression;
    dayOfMonth?: CronScheduleExpression;
    month?: CronScheduleExpression;
    dayOfWeek?: CronScheduleExpression;
  }

  /** **UNSTABLE**: New API, yet to be vetted.
   *
   * Create a cron job that will periodically execute the provided handler
   * callback based on the specified schedule.
   *
   * ```ts
   * Deno.cron("sample cron", "20 * * * *", () => {
   *   console.log("cron job executed");
   * });
   * ```
   *
   * ```ts
   * Deno.cron("sample cron", { hour: { every: 6 } }, () => {
   *   console.log("cron job executed");
   * });
   * ```
   *
   * `schedule` can be a string in the Unix cron format or in JSON format
   * as specified by interface {@linkcode CronSchedule}, where time is specified
   * using UTC time zone.
   *
   * @category Cron
   */
  export function cron(
    name: string,
    schedule: string | CronSchedule,
    handler: () => Promise<void> | void,
  ): Promise<void>;

  /** **UNSTABLE**: New API, yet to be vetted.
   *
   * Create a cron job that will periodically execute the provided handler
   * callback based on the specified schedule.
   *
   * ```ts
   * Deno.cron("sample cron", "20 * * * *", {
   *   backoffSchedule: [10, 20]
   * }, () => {
   *   console.log("cron job executed");
   * });
   * ```
   *
   * `schedule` can be a string in the Unix cron format or in JSON format
   * as specified by interface {@linkcode CronSchedule}, where time is specified
   * using UTC time zone.
   *
   * `backoffSchedule` option can be used to specify the retry policy for failed
   * executions. Each element in the array represents the number of milliseconds
   * to wait before retrying the execution. For example, `[1000, 5000, 10000]`
   * means that a failed execution will be retried at most 3 times, with 1
   * second, 5 seconds, and 10 seconds delay between each retry.
   *
   * @category Cron
   */
  export function cron(
    name: string,
    schedule: string | CronSchedule,
    options: { backoffSchedule?: number[]; signal?: AbortSignal },
    handler: () => Promise<void> | void,
  ): Promise<void>;

  /** **UNSTABLE**: New API, yet to be vetted.
   *
   * Create a cron job that will periodically execute the provided handler
   * callback based on the specified schedule.
   *
   * `schedule` can be a string in the Unix cron format or in JSON format
   * as specified by interface {@linkcode CronSchedule}, where time is specified
   * using UTC time zone.
   *
   * ```ts
   * Deno.cron("sample cron", "20 * * * *", () => {
   *   console.log("cron job executed");
   * });
   * ```
   * `backoffSchedule` option can be used to specify the retry policy for failed
   * executions. Each element in the array represents the number of milliseconds
   * to wait before retrying the execution. For example, `[1000, 5000, 10000]`
   * means that a failed execution will be retried at most 3 times, with 1
   * second, 5 seconds, and 10 seconds delay between each retry.
   *
   * @category Cron
   * @deprecated Use other {@linkcode cron} overloads instead. This overload
   * will be removed in the future.
   */
  export function cron(
    name: string,
    schedule: string | CronSchedule,
    handler: () => Promise<void> | void,
    options: { backoffSchedule?: number[]; signal?: AbortSignal },
  ): Promise<void>;

  /** **UNSTABLE**: New API, yet to be vetted.
   *
   * A key to be persisted in a {@linkcode Deno.Kv}. A key is a sequence
   * of {@linkcode Deno.KvKeyPart}s.
   *
   * Keys are ordered lexicographically by their parts. The first part is the
   * most significant, and the last part is the least significant. The order of
   * the parts is determined by both the type and the value of the part. The
   * relative significance of the types can be found in documentation for the
   * {@linkcode Deno.KvKeyPart} type.
   *
   * Keys have a maximum size of 2048 bytes serialized. If the size of the key
   * exceeds this limit, an error will be thrown on the operation that this key
   * was passed to.
   *
   * @category KV
   */
  export type KvKey = readonly KvKeyPart[];

  /** **UNSTABLE**: New API, yet to be vetted.
   *
   * A single part of a {@linkcode Deno.KvKey}. Parts are ordered
   * lexicographically, first by their type, and within a given type by their
   * value.
   *
   * The ordering of types is as follows:
   *
   * 1. `Uint8Array`
   * 2. `string`
   * 3. `number`
   * 4. `bigint`
   * 5. `boolean`
   *
   * Within a given type, the ordering is as follows:
   *
   * - `Uint8Array` is ordered by the byte ordering of the array
   * - `string` is ordered by the byte ordering of the UTF-8 encoding of the
   *   string
   * - `number` is ordered following this pattern: `-NaN`
   *   < `-Infinity` < `-100.0` < `-1.0` < -`0.5` < `-0.0` < `0.0` < `0.5`
   *   < `1.0` < `100.0` < `Infinity` < `NaN`
   * - `bigint` is ordered by mathematical ordering, with the largest negative
   *   number being the least first value, and the largest positive number
   *   being the last value
   * - `boolean` is ordered by `false` < `true`
   *
   * This means that the part `1.0` (a number) is ordered before the part `2.0`
   * (also a number), but is greater than the part `0n` (a bigint), because
   * `1.0` is a number and `0n` is a bigint, and type ordering has precedence
   * over the ordering of values within a type.
   *
   * @category KV
   */
  export type KvKeyPart =
    | Uint8Array
    | string
    | number
    | bigint
    | boolean
    | symbol;

  /** **UNSTABLE**: New API, yet to be vetted.
   *
   * Consistency level of a KV operation.
   *
   * - `strong` - This operation must be strongly-consistent.
   * - `eventual` - Eventually-consistent behavior is allowed.
   *
   * @category KV
   */
  export type KvConsistencyLevel = "strong" | "eventual";

  /** **UNSTABLE**: New API, yet to be vetted.
   *
   * A selector that selects the range of data returned by a list operation on a
   * {@linkcode Deno.Kv}.
   *
   * The selector can either be a prefix selector or a range selector. A prefix
   * selector selects all keys that start with the given prefix (optionally
   * starting at a given key). A range selector selects all keys that are
   * lexicographically between the given start and end keys.
   *
   * @category KV
   */
  export type KvListSelector =
    | { prefix: KvKey }
    | { prefix: KvKey; start: KvKey }
    | { prefix: KvKey; end: KvKey }
    | { start: KvKey; end: KvKey };

  /** **UNSTABLE**: New API, yet to be vetted.
   *
   * A mutation to a key in a {@linkcode Deno.Kv}. A mutation is a
   * combination of a key, a value, and a type. The type determines how the
   * mutation is applied to the key.
   *
   * - `set` - Sets the value of the key to the given value, overwriting any
   *   existing value. Optionally an `expireIn` option can be specified to
   *   set a time-to-live (TTL) for the key. The TTL is specified in
   *   milliseconds, and the key will be deleted from the database at earliest
   *   after the specified number of milliseconds have elapsed. Once the
   *   specified duration has passed, the key may still be visible for some
   *   additional time. If the `expireIn` option is not specified, the key will
   *   not expire.
   * - `delete` - Deletes the key from the database. The mutation is a no-op if
   *   the key does not exist.
   * - `sum` - Adds the given value to the existing value of the key. Both the
   *   value specified in the mutation, and any existing value must be of type
   *   `Deno.KvU64`. If the key does not exist, the value is set to the given
   *   value (summed with 0). If the result of the sum overflows an unsigned
   *   64-bit integer, the result is wrapped around.
   * - `max` - Sets the value of the key to the maximum of the existing value
   *   and the given value. Both the value specified in the mutation, and any
   *   existing value must be of type `Deno.KvU64`. If the key does not exist,
   *   the value is set to the given value.
   * - `min` - Sets the value of the key to the minimum of the existing value
   *   and the given value. Both the value specified in the mutation, and any
   *   existing value must be of type `Deno.KvU64`. If the key does not exist,
   *   the value is set to the given value.
   *
   * @category KV
   */
  export type KvMutation =
    & { key: KvKey }
    & (
      | { type: "set"; value: unknown; expireIn?: number }
      | { type: "delete" }
      | { type: "sum"; value: KvU64 }
      | { type: "max"; value: KvU64 }
      | { type: "min"; value: KvU64 }
    );

  /** **UNSTABLE**: New API, yet to be vetted.
   *
   * An iterator over a range of data entries in a {@linkcode Deno.Kv}.
   *
   * The cursor getter returns the cursor that can be used to resume the
   * iteration from the current position in the future.
   *
   * @category KV
   */
  export class KvListIterator<T> implements AsyncIterableIterator<KvEntry<T>> {
    /**
     * Returns the cursor of the current position in the iteration. This cursor
     * can be used to resume the iteration from the current position in the
     * future by passing it to the `cursor` option of the `list` method.
     */
    get cursor(): string;

    next(): Promise<IteratorResult<KvEntry<T>, undefined>>;
    [Symbol.asyncIterator](): AsyncIterableIterator<KvEntry<T>>;
  }

  /** **UNSTABLE**: New API, yet to be vetted.
   *
   * A versioned pair of key and value in a {@linkcode Deno.Kv}.
   *
   * The `versionstamp` is a string that represents the current version of the
   * key-value pair. It can be used to perform atomic operations on the KV store
   * by passing it to the `check` method of a {@linkcode Deno.AtomicOperation}.
   *
   * @category KV
   */
  export type KvEntry<T> = { key: KvKey; value: T; versionstamp: string };

  /**
   * **UNSTABLE**: New API, yet to be vetted.
   *
   * An optional versioned pair of key and value in a {@linkcode Deno.Kv}.
   *
   * This is the same as a {@linkcode KvEntry}, but the `value` and `versionstamp`
   * fields may be `null` if no value exists for the given key in the KV store.
   *
   * @category KV
   */
  export type KvEntryMaybe<T> = KvEntry<T> | {
    key: KvKey;
    value: null;
    versionstamp: null;
  };

  /** **UNSTABLE**: New API, yet to be vetted.
   *
   * Options for listing key-value pairs in a {@linkcode Deno.Kv}.
   *
   * @category KV
   */
  export interface KvListOptions {
    /**
     * The maximum number of key-value pairs to return. If not specified, all
     * matching key-value pairs will be returned.
     */
    limit?: number;
    /**
     * The cursor to resume the iteration from. If not specified, the iteration
     * will start from the beginning.
     */
    cursor?: string;
    /**
     * Whether to reverse the order of the returned key-value pairs. If not
     * specified, the order will be ascending from the start of the range as per
     * the lexicographical ordering of the keys. If `true`, the order will be
     * descending from the end of the range.
     *
     * The default value is `false`.
     */
    reverse?: boolean;
    /**
     * The consistency level of the list operation. The default consistency
     * level is "strong". Some use cases can benefit from using a weaker
     * consistency level. For more information on consistency levels, see the
     * documentation for {@linkcode Deno.KvConsistencyLevel}.
     *
     * List operations are performed in batches (in sizes specified by the
     * `batchSize` option). The consistency level of the list operation is
     * applied to each batch individually. This means that while each batch is
     * guaranteed to be consistent within itself, the entire list operation may
     * not be consistent across batches because a mutation may be applied to a
     * key-value pair between batches, in a batch that has already been returned
     * by the list operation.
     */
    consistency?: KvConsistencyLevel;
    /**
     * The size of the batches in which the list operation is performed. Larger
     * or smaller batch sizes may positively or negatively affect the
     * performance of a list operation depending on the specific use case and
     * iteration behavior. Slow iterating queries may benefit from using a
     * smaller batch size for increased overall consistency, while fast
     * iterating queries may benefit from using a larger batch size for better
     * performance.
     *
     * The default batch size is equal to the `limit` option, or 100 if this is
     * unset. The maximum value for this option is 500. Larger values will be
     * clamped.
     */
    batchSize?: number;
  }

  /** @category KV */
  export interface KvCommitResult {
    ok: true;
    /** The versionstamp of the value committed to KV. */
    versionstamp: string;
  }

  /** @category KV */
  export interface KvCommitError {
    ok: false;
  }

  /** **UNSTABLE**: New API, yet to be vetted.
   *
   * A check to perform as part of a {@linkcode Deno.AtomicOperation}. The check
   * will fail if the versionstamp for the key-value pair in the KV store does
   * not match the given versionstamp. A check with a `null` versionstamp checks
   * that the key-value pair does not currently exist in the KV store.
   *
   * @category KV
   */
  export interface AtomicCheck {
    key: KvKey;
    versionstamp: string | null;
  }

  /** **UNSTABLE**: New API, yet to be vetted.
   *
   * An operation on a {@linkcode Deno.Kv} that can be performed
   * atomically. Atomic operations do not auto-commit, and must be committed
   * explicitly by calling the `commit` method.
   *
   * Atomic operations can be used to perform multiple mutations on the KV store
   * in a single atomic transaction. They can also be used to perform
   * conditional mutations by specifying one or more
   * {@linkcode Deno.AtomicCheck}s that ensure that a mutation is only performed
   * if the key-value pair in the KV has a specific versionstamp. If any of the
   * checks fail, the entire operation will fail and no mutations will be made.
   *
   * The ordering of mutations is guaranteed to be the same as the ordering of
   * the mutations specified in the operation. Checks are performed before any
   * mutations are performed. The ordering of checks is unobservable.
   *
   * Atomic operations can be used to implement optimistic locking, where a
   * mutation is only performed if the key-value pair in the KV store has not
   * been modified since the last read. This can be done by specifying a check
   * that ensures that the versionstamp of the key-value pair matches the
   * versionstamp that was read. If the check fails, the mutation will not be
   * performed and the operation will fail. One can then retry the read-modify-
   * write operation in a loop until it succeeds.
   *
   * The `commit` method of an atomic operation returns a value indicating
   * whether checks passed and mutations were performed. If the operation failed
   * because of a failed check, the return value will be a
   * {@linkcode Deno.KvCommitError} with an `ok: false` property. If the
   * operation failed for any other reason (storage error, invalid value, etc.),
   * an exception will be thrown. If the operation succeeded, the return value
   * will be a {@linkcode Deno.KvCommitResult} object with a `ok: true` property
   * and the versionstamp of the value committed to KV.

   *
   * @category KV
   */
  export class AtomicOperation {
    /**
     * Add to the operation a check that ensures that the versionstamp of the
     * key-value pair in the KV store matches the given versionstamp. If the
     * check fails, the entire operation will fail and no mutations will be
     * performed during the commit.
     */
    check(...checks: AtomicCheck[]): this;
    /**
     * Add to the operation a mutation that performs the specified mutation on
     * the specified key if all checks pass during the commit. The types and
     * semantics of all available mutations are described in the documentation
     * for {@linkcode Deno.KvMutation}.
     */
    mutate(...mutations: KvMutation[]): this;
    /**
     * Shortcut for creating a `sum` mutation. This method wraps `n` in a
     * {@linkcode Deno.KvU64}, so the value of `n` must be in the range
     * `[0, 2^64-1]`.
     */
    sum(key: KvKey, n: bigint): this;
    /**
     * Shortcut for creating a `min` mutation. This method wraps `n` in a
     * {@linkcode Deno.KvU64}, so the value of `n` must be in the range
     * `[0, 2^64-1]`.
     */
    min(key: KvKey, n: bigint): this;
    /**
     * Shortcut for creating a `max` mutation. This method wraps `n` in a
     * {@linkcode Deno.KvU64}, so the value of `n` must be in the range
     * `[0, 2^64-1]`.
     */
    max(key: KvKey, n: bigint): this;
    /**
     * Add to the operation a mutation that sets the value of the specified key
     * to the specified value if all checks pass during the commit.
     *
     * Optionally an `expireIn` option can be specified to set a time-to-live
     * (TTL) for the key. The TTL is specified in milliseconds, and the key will
     * be deleted from the database at earliest after the specified number of
     * milliseconds have elapsed. Once the specified duration has passed, the
     * key may still be visible for some additional time. If the `expireIn`
     * option is not specified, the key will not expire.
     */
    set(key: KvKey, value: unknown, options?: { expireIn?: number }): this;
    /**
     * Add to the operation a mutation that deletes the specified key if all
     * checks pass during the commit.
     */
    delete(key: KvKey): this;
    /**
     * Add to the operation a mutation that enqueues a value into the queue
     * if all checks pass during the commit.
     */
    enqueue(
      value: unknown,
      options?: {
        delay?: number;
        keysIfUndelivered?: Deno.KvKey[];
        backoffSchedule?: number[];
      },
    ): this;
    /**
     * Commit the operation to the KV store. Returns a value indicating whether
     * checks passed and mutations were performed. If the operation failed
     * because of a failed check, the return value will be a {@linkcode
     * Deno.KvCommitError} with an `ok: false` property. If the operation failed
     * for any other reason (storage error, invalid value, etc.), an exception
     * will be thrown. If the operation succeeded, the return value will be a
     * {@linkcode Deno.KvCommitResult} object with a `ok: true` property and the
     * versionstamp of the value committed to KV.
     *
     * If the commit returns `ok: false`, one may create a new atomic operation
     * with updated checks and mutations and attempt to commit it again. See the
     * note on optimistic locking in the documentation for
     * {@linkcode Deno.AtomicOperation}.
     */
    commit(): Promise<KvCommitResult | KvCommitError>;
  }

  /** **UNSTABLE**: New API, yet to be vetted.
   *
   * A key-value database that can be used to store and retrieve data.
   *
   * Data is stored as key-value pairs, where the key is a {@linkcode Deno.KvKey}
   * and the value is an arbitrary structured-serializable JavaScript value.
   * Keys are ordered lexicographically as described in the documentation for
   * {@linkcode Deno.KvKey}. Keys are unique within a database, and the last
   * value set for a given key is the one that is returned when reading the
   * key. Keys can be deleted from the database, in which case they will no
   * longer be returned when reading keys.
   *
   * Values can be any structured-serializable JavaScript value (objects,
   * arrays, strings, numbers, etc.). The special value {@linkcode Deno.KvU64}
   * can be used to store 64-bit unsigned integers in the database. This special
   * value can not be nested within other objects or arrays. In addition to the
   * regular database mutation operations, the unsigned 64-bit integer value
   * also supports `sum`, `max`, and `min` mutations.
   *
   * Keys are versioned on write by assigning the key an ever-increasing
   * "versionstamp". The versionstamp represents the version of a key-value pair
   * in the database at some point in time, and can be used to perform
   * transactional operations on the database without requiring any locking.
   * This is enabled by atomic operations, which can have conditions that ensure
   * that the operation only succeeds if the versionstamp of the key-value pair
   * matches an expected versionstamp.
   *
   * Keys have a maximum length of 2048 bytes after serialization. Values have a
   * maximum length of 64 KiB after serialization. Serialization of both keys
   * and values is somewhat opaque, but one can usually assume that the
   * serialization of any value is about the same length as the resulting string
   * of a JSON serialization of that same value. If theses limits are exceeded,
   * an exception will be thrown.
   *
   * @category KV
   */
  export class Kv implements Disposable {
    /**
     * Retrieve the value and versionstamp for the given key from the database
     * in the form of a {@linkcode Deno.KvEntryMaybe}. If no value exists for
     * the key, the returned entry will have a `null` value and versionstamp.
     *
     * ```ts
     * const db = await Deno.openKv();
     * const result = await db.get(["foo"]);
     * result.key; // ["foo"]
     * result.value; // "bar"
     * result.versionstamp; // "00000000000000010000"
     * ```
     *
     * The `consistency` option can be used to specify the consistency level
     * for the read operation. The default consistency level is "strong". Some
     * use cases can benefit from using a weaker consistency level. For more
     * information on consistency levels, see the documentation for
     * {@linkcode Deno.KvConsistencyLevel}.
     */
    get<T = unknown>(
      key: KvKey,
      options?: { consistency?: KvConsistencyLevel },
    ): Promise<KvEntryMaybe<T>>;

    /**
     * Retrieve multiple values and versionstamps from the database in the form
     * of an array of {@linkcode Deno.KvEntryMaybe} objects. The returned array
     * will have the same length as the `keys` array, and the entries will be in
     * the same order as the keys. If no value exists for a given key, the
     * returned entry will have a `null` value and versionstamp.
     *
     * ```ts
     * const db = await Deno.openKv();
     * const result = await db.getMany([["foo"], ["baz"]]);
     * result[0].key; // ["foo"]
     * result[0].value; // "bar"
     * result[0].versionstamp; // "00000000000000010000"
     * result[1].key; // ["baz"]
     * result[1].value; // null
     * result[1].versionstamp; // null
     * ```
     *
     * The `consistency` option can be used to specify the consistency level
     * for the read operation. The default consistency level is "strong". Some
     * use cases can benefit from using a weaker consistency level. For more
     * information on consistency levels, see the documentation for
     * {@linkcode Deno.KvConsistencyLevel}.
     */
    getMany<T extends readonly unknown[]>(
      keys: readonly [...{ [K in keyof T]: KvKey }],
      options?: { consistency?: KvConsistencyLevel },
    ): Promise<{ [K in keyof T]: KvEntryMaybe<T[K]> }>;
    /**
     * Set the value for the given key in the database. If a value already
     * exists for the key, it will be overwritten.
     *
     * ```ts
     * const db = await Deno.openKv();
     * await db.set(["foo"], "bar");
     * ```
     *
     * Optionally an `expireIn` option can be specified to set a time-to-live
     * (TTL) for the key. The TTL is specified in milliseconds, and the key will
     * be deleted from the database at earliest after the specified number of
     * milliseconds have elapsed. Once the specified duration has passed, the
     * key may still be visible for some additional time. If the `expireIn`
     * option is not specified, the key will not expire.
     */
    set(
      key: KvKey,
      value: unknown,
      options?: { expireIn?: number },
    ): Promise<KvCommitResult>;

    /**
     * Delete the value for the given key from the database. If no value exists
     * for the key, this operation is a no-op.
     *
     * ```ts
     * const db = await Deno.openKv();
     * await db.delete(["foo"]);
     * ```
     */
    delete(key: KvKey): Promise<void>;

    /**
     * Retrieve a list of keys in the database. The returned list is an
     * {@linkcode Deno.KvListIterator} which can be used to iterate over the
     * entries in the database.
     *
     * Each list operation must specify a selector which is used to specify the
     * range of keys to return. The selector can either be a prefix selector, or
     * a range selector:
     *
     * - A prefix selector selects all keys that start with the given prefix of
     *   key parts. For example, the selector `["users"]` will select all keys
     *   that start with the prefix `["users"]`, such as `["users", "alice"]`
     *   and `["users", "bob"]`. Note that you can not partially match a key
     *   part, so the selector `["users", "a"]` will not match the key
     *   `["users", "alice"]`. A prefix selector may specify a `start` key that
     *   is used to skip over keys that are lexicographically less than the
     *   start key.
     * - A range selector selects all keys that are lexicographically between
     *   the given start and end keys (including the start, and excluding the
     *   end). For example, the selector `["users", "a"], ["users", "n"]` will
     *   select all keys that start with the prefix `["users"]` and have a
     *   second key part that is lexicographically between `a` and `n`, such as
     *   `["users", "alice"]`, `["users", "bob"]`, and `["users", "mike"]`, but
     *   not `["users", "noa"]` or `["users", "zoe"]`.
     *
     * ```ts
     * const db = await Deno.openKv();
     * const entries = db.list({ prefix: ["users"] });
     * for await (const entry of entries) {
     *   entry.key; // ["users", "alice"]
     *   entry.value; // { name: "Alice" }
     *   entry.versionstamp; // "00000000000000010000"
     * }
     * ```
     *
     * The `options` argument can be used to specify additional options for the
     * list operation. See the documentation for {@linkcode Deno.KvListOptions}
     * for more information.
     */
    list<T = unknown>(
      selector: KvListSelector,
      options?: KvListOptions,
    ): KvListIterator<T>;

    /**
     * Add a value into the database queue to be delivered to the queue
     * listener via {@linkcode Deno.Kv.listenQueue}.
     *
     * ```ts
     * const db = await Deno.openKv();
     * await db.enqueue("bar");
     * ```
     *
     * The `delay` option can be used to specify the delay (in milliseconds)
     * of the value delivery. The default delay is 0, which means immediate
     * delivery.
     *
     * ```ts
     * const db = await Deno.openKv();
     * await db.enqueue("bar", { delay: 60000 });
     * ```
     *
     * The `keysIfUndelivered` option can be used to specify the keys to
     * be set if the value is not successfully delivered to the queue
     * listener after several attempts. The values are set to the value of
     * the queued message.
     *
     * The `backoffSchedule` option can be used to specify the retry policy for
     * failed message delivery. Each element in the array represents the number of
     * milliseconds to wait before retrying the delivery. For example,
     * `[1000, 5000, 10000]` means that a failed delivery will be retried
     * at most 3 times, with 1 second, 5 seconds, and 10 seconds delay
     * between each retry.
     *
     * ```ts
     * const db = await Deno.openKv();
     * await db.enqueue("bar", {
     *   keysIfUndelivered: [["foo", "bar"]],
     *   backoffSchedule: [1000, 5000, 10000],
     * });
     * ```
     */
    enqueue(
      value: unknown,
      options?: {
        delay?: number;
        keysIfUndelivered?: Deno.KvKey[];
        backoffSchedule?: number[];
      },
    ): Promise<KvCommitResult>;

    /**
     * Listen for queue values to be delivered from the database queue, which
     * were enqueued with {@linkcode Deno.Kv.enqueue}. The provided handler
     * callback is invoked on every dequeued value. A failed callback
     * invocation is automatically retried multiple times until it succeeds
     * or until the maximum number of retries is reached.
     *
     * ```ts
     * const db = await Deno.openKv();
     * db.listenQueue(async (msg: unknown) => {
     *   await db.set(["foo"], msg);
     * });
     * ```
     */
    listenQueue(
      handler: (value: unknown) => Promise<void> | void,
    ): Promise<void>;

    /**
     * Create a new {@linkcode Deno.AtomicOperation} object which can be used to
     * perform an atomic transaction on the database. This does not perform any
     * operations on the database - the atomic transaction must be committed
     * explicitly using the {@linkcode Deno.AtomicOperation.commit} method once
     * all checks and mutations have been added to the operation.
     */
    atomic(): AtomicOperation;

    /**
     * Watch for changes to the given keys in the database. The returned stream
     * is a {@linkcode ReadableStream} that emits a new value whenever any of
     * the watched keys change their versionstamp. The emitted value is an array
     * of {@linkcode Deno.KvEntryMaybe} objects, with the same length and order
     * as the `keys` array. If no value exists for a given key, the returned
     * entry will have a `null` value and versionstamp.
     *
     * The returned stream does not return every single intermediate state of
     * the watched keys, but rather only keeps you up to date with the latest
     * state of the keys. This means that if a key is modified multiple times
     * quickly, you may not receive a notification for every single change, but
     * rather only the latest state of the key.
     *
     * ```ts
     * const db = await Deno.openKv();
     *
     * const stream = db.watch([["foo"], ["bar"]]);
     * for await (const entries of stream) {
     *   entries[0].key; // ["foo"]
     *   entries[0].value; // "bar"
     *   entries[0].versionstamp; // "00000000000000010000"
     *   entries[1].key; // ["bar"]
     *   entries[1].value; // null
     *   entries[1].versionstamp; // null
     * }
     * ```
     *
     * The `options` argument can be used to specify additional options for the
     * watch operation. The `raw` option can be used to specify whether a new
     * value should be emitted whenever a mutation occurs on any of the watched
     * keys (even if the value of the key does not change, such as deleting a
     * deleted key), or only when entries have observably changed in some way.
     * When `raw: true` is used, it is possible for the stream to occasionally
     * emit values even if no mutations have occurred on any of the watched
     * keys. The default value for this option is `false`.
     */
    watch<T extends readonly unknown[]>(
      keys: readonly [...{ [K in keyof T]: KvKey }],
      options?: { raw?: boolean },
    ): ReadableStream<{ [K in keyof T]: KvEntryMaybe<T[K]> }>;

    /**
     * Close the database connection. This will prevent any further operations
     * from being performed on the database, and interrupt any in-flight
     * operations immediately.
     */
    close(): void;

    /**
     * Get a symbol that represents the versionstamp of the current atomic
     * operation. This symbol can be used as the last part of a key in
     * `.set()`, both directly on the `Kv` object and on an `AtomicOperation`
     * object created from this `Kv` instance.
     */
    commitVersionstamp(): symbol;

    [Symbol.dispose](): void;
  }

  /** **UNSTABLE**: New API, yet to be vetted.
   *
   * Wrapper type for 64-bit unsigned integers for use as values in a
   * {@linkcode Deno.Kv}.
   *
   * @category KV
   */
  export class KvU64 {
    /** Create a new `KvU64` instance from the given bigint value. If the value
     * is signed or greater than 64-bits, an error will be thrown. */
    constructor(value: bigint);
    /** The value of this unsigned 64-bit integer, represented as a bigint. */
    readonly value: bigint;
  }

  /**
   * A namespace containing runtime APIs available in Jupyter notebooks.
   *
   * When accessed outside of Jupyter notebook context an error will be thrown.
   *
   * @category Jupyter */
  export namespace jupyter {
    /** @category Jupyter */
    export interface DisplayOptions {
      raw?: boolean;
      update?: boolean;
      display_id?: string;
    }

    type VegaObject = {
      $schema: string;
      [key: string]: unknown;
    };

    /**
     * A collection of supported media types and data for Jupyter frontends.
     *
     * @category Jupyter
     */
    export type MediaBundle = {
      "text/plain"?: string;
      "text/html"?: string;
      "image/svg+xml"?: string;
      "text/markdown"?: string;
      "application/javascript"?: string;

      // Images (per Jupyter spec) must be base64 encoded. We could _allow_
      // accepting Uint8Array or ArrayBuffer within `display` calls, however we still
      // must encode them for jupyter.
      "image/png"?: string; // WISH: Uint8Array | ArrayBuffer
      "image/jpeg"?: string; // WISH: Uint8Array | ArrayBuffer
      "image/gif"?: string; // WISH: Uint8Array | ArrayBuffer
      "application/pdf"?: string; // WISH: Uint8Array | ArrayBuffer

      // NOTE: all JSON types must be objects at the top level (no arrays, strings, or other primitives)
      "application/json"?: object;
      "application/geo+json"?: object;
      "application/vdom.v1+json"?: object;
      "application/vnd.plotly.v1+json"?: object;
      "application/vnd.vega.v5+json"?: VegaObject;
      "application/vnd.vegalite.v4+json"?: VegaObject;
      "application/vnd.vegalite.v5+json"?: VegaObject;

      // Must support a catch all for custom media types / mimetypes
      [key: string]: string | object | undefined;
    };

    /** @category Jupyter */
    export const $display: unique symbol;

    /** @category Jupyter */
    export type Displayable = {
      [$display]: () => MediaBundle | Promise<MediaBundle>;
    };

    /**
     * Display function for Jupyter Deno Kernel.
     * Mimics the behavior of IPython's `display(obj, raw=True)` function to allow
     * asynchronous displaying of objects in Jupyter.
     *
     * @param obj - The object to be displayed
     * @param options - Display options with a default { raw: true }
     * @category Jupyter
     */
    export function display(obj: unknown, options?: DisplayOptions): void;

    /**
     * Show Markdown in Jupyter frontends with a tagged template function.
     *
     * Takes a template string and returns a displayable object for Jupyter frontends.
     *
     * @example
     * Create a Markdown view.
     *
     * ```typescript
     * const { md } = Deno.jupyter;
     * md`# Notebooks in TypeScript via Deno ![Deno logo](https://github.com/denoland.png?size=32)
     *
     * * TypeScript ${Deno.version.typescript}
     * * V8 ${Deno.version.v8}
     * * Deno ${Deno.version.deno}
     *
     * Interactive compute with Jupyter _built into Deno_!
     * `
     * ```
     *
     * @category Jupyter
     */
    export function md(
      strings: TemplateStringsArray,
      ...values: unknown[]
    ): Displayable;

    /**
     * Show HTML in Jupyter frontends with a tagged template function.
     *
     * Takes a template string and returns a displayable object for Jupyter frontends.
     *
     * @example
     * Create an HTML view.
     * ```typescript
     * const { html } = Deno.jupyter;
     * html`<h1>Hello, world!</h1>`
     * ```
     *
     * @category Jupyter
     */
    export function html(
      strings: TemplateStringsArray,
      ...values: unknown[]
    ): Displayable;

    /**
     * SVG Tagged Template Function.
     *
     * Takes a template string and returns a displayable object for Jupyter frontends.
     *
     * Example usage:
     *
     * svg`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
     *      <circle cx="50" cy="50" r="40" stroke="green" stroke-width="4" fill="yellow" />
     *    </svg>`
     *
     * @category Jupyter
     */
    export function svg(
      strings: TemplateStringsArray,
      ...values: unknown[]
    ): Displayable;

    /**
     * Format an object for displaying in Deno
     *
     * @param obj - The object to be displayed
     * @returns MediaBundle
     *
     * @category Jupyter
     */
    export function format(obj: unknown): MediaBundle;

    /**
     * Broadcast a message on IO pub channel.
     *
     * ```
     * await Deno.jupyter.broadcast("display_data", {
     *   data: { "text/html": "<b>Processing.</b>" },
     *   metadata: {},
     *   transient: { display_id: "progress" }
     * });
     *
     * await new Promise((resolve) => setTimeout(resolve, 500));
     *
     * await Deno.jupyter.broadcast("update_display_data", {
     *   data: { "text/html": "<b>Processing..</b>" },
     *   metadata: {},
     *   transient: { display_id: "progress" }
     * });
     * ```
     *
     * @category Jupyter */
    export function broadcast(
      msgType: string,
      content: Record<string, unknown>,
      extra?: {
        metadata?: Record<string, unknown>;
        buffers?: Uint8Array[];
      },
    ): Promise<void>;
  }
}

/** **UNSTABLE**: New API, yet to be vetted.
 *
 * The [Fetch API](https://developer.mozilla.org/en-US/docs/Web/API/Fetch_API)
 * which also supports setting a {@linkcode Deno.HttpClient} which provides a
 * way to connect via proxies and use custom TLS certificates.
 *
 * @tags allow-net, allow-read
 * @category Fetch API
 */
declare function fetch(
  input: Request | URL | string,
  init?: RequestInit & { client: Deno.HttpClient },
): Promise<Response>;

/** **UNSTABLE**: New API, yet to be vetted.
 *
 * @category Web Workers
 */
declare interface WorkerOptions {
  /** **UNSTABLE**: New API, yet to be vetted.
   *
   * Configure permissions options to change the level of access the worker will
   * have. By default it will have no permissions. Note that the permissions
   * of a worker can't be extended beyond its parent's permissions reach.
   *
   * - `"inherit"` will take the permissions of the thread the worker is created
   *   in.
   * - `"none"` will use the default behavior and have no permission
   * - A list of routes can be provided that are relative to the file the worker
   *   is created in to limit the access of the worker (read/write permissions
   *   only)
   *
   * Example:
   *
   * ```ts
   * // mod.ts
   * const worker = new Worker(
   *   new URL("deno_worker.ts", import.meta.url).href, {
   *     type: "module",
   *     deno: {
   *       permissions: {
   *         read: true,
   *       },
   *     },
   *   }
   * );
   * ```
   */
  deno?: {
    /** Set to `"none"` to disable all the permissions in the worker. */
    permissions?: Deno.PermissionOptions;
  };
}

/** **UNSTABLE**: New API, yet to be vetted.
 *
 * @category Web Sockets
 */
declare interface WebSocketStreamOptions {
  protocols?: string[];
  signal?: AbortSignal;
  headers?: HeadersInit;
}

/** **UNSTABLE**: New API, yet to be vetted.
 *
 * @category Web Sockets
 */
declare interface WebSocketConnection {
  readable: ReadableStream<string | Uint8Array>;
  writable: WritableStream<string | Uint8Array>;
  extensions: string;
  protocol: string;
}

/** **UNSTABLE**: New API, yet to be vetted.
 *
 * @category Web Sockets
 */
declare interface WebSocketCloseInfo {
  code?: number;
  reason?: string;
}

/** **UNSTABLE**: New API, yet to be vetted.
 *
 * @tags allow-net
 * @category Web Sockets
 */
declare interface WebSocketStream {
  url: string;
  opened: Promise<WebSocketConnection>;
  closed: Promise<WebSocketCloseInfo>;
  close(closeInfo?: WebSocketCloseInfo): void;
}

/** **UNSTABLE**: New API, yet to be vetted.
 *
 * @tags allow-net
 * @category Web Sockets
 */
declare var WebSocketStream: {
  readonly prototype: WebSocketStream;
  new (url: string, options?: WebSocketStreamOptions): WebSocketStream;
};
