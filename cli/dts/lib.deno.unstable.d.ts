// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

/// <reference no-default-lib="true" />
/// <reference lib="deno.ns" />

declare namespace Deno {
  export {}; // stop default export type behavior

  /** @category Testing */
  export interface BenchDefinition {
    fn: () => void | Promise<void>;
    name: string;
    ignore?: boolean;
    /** Group name for the benchmark.
     * Grouped benchmarks produce a time summary */
    group?: string;
    /** Benchmark should be used as the baseline for other benchmarks
     * If there are multiple baselines in a group, the first one is used as the baseline */
    baseline?: boolean;
    /** If at least one bench has `only` set to true, only run benches that have
     * `only` set to true and fail the bench suite. */
    only?: boolean;
    /** Ensure the bench case does not prematurely cause the process to exit,
     * for example via a call to `Deno.exit`. Defaults to true. */
    sanitizeExit?: boolean;

    /** Specifies the permissions that should be used to run the bench.
     * Set this to "inherit" to keep the calling thread's permissions.
     * Set this to "none" to revoke all permissions.
     *
     * Defaults to "inherit".
     */
    permissions?: Deno.PermissionOptions;
  }

  /** Register a bench which will be run when `deno bench` is used on the command
   * line and the containing module looks like a bench module.
   * `fn` can be async if required.
   * ```ts
   * import {assert, fail, assertEquals} from "https://deno.land/std/testing/asserts.ts";
   *
   * Deno.bench({
   *   name: "example test",
   *   fn(): void {
   *     assertEquals("world", "world");
   *   },
   * });
   *
   * Deno.bench({
   *   name: "example ignored test",
   *   ignore: Deno.build.os === "windows",
   *   fn(): void {
   *     // This test is ignored only on Windows machines
   *   },
   * });
   *
   * Deno.bench({
   *   name: "example async test",
   *   async fn() {
   *     const decoder = new TextDecoder("utf-8");
   *     const data = await Deno.readFile("hello_world.txt");
   *     assertEquals(decoder.decode(data), "Hello world");
   *   }
   * });
   * ```
   *
   * @category Testing
   */
  export function bench(t: BenchDefinition): void;

  /** Register a bench which will be run when `deno bench` is used on the command
   * line and the containing module looks like a bench module.
   * `fn` can be async if required.
   *
   * ```ts
   * import {assert, fail, assertEquals} from "https://deno.land/std/testing/asserts.ts";
   *
   * Deno.bench("My test description", (): void => {
   *   assertEquals("hello", "hello");
   * });
   *
   * Deno.bench("My async test description", async (): Promise<void> => {
   *   const decoder = new TextDecoder("utf-8");
   *   const data = await Deno.readFile("hello_world.txt");
   *   assertEquals(decoder.decode(data), "Hello world");
   * });
   * ```
   *
   * @category Testing
   */
  export function bench(
    name: string,
    fn: () => void | Promise<void>,
  ): void;

  /** Register a bench which will be run when `deno bench` is used on the command
   * line and the containing module looks like a bench module.
   * `fn` can be async if required. Declared function must have a name.
   *
   * ```ts
   * import {assert, fail, assertEquals} from "https://deno.land/std/testing/asserts.ts";
   *
   * Deno.bench(function myTestName(): void {
   *   assertEquals("hello", "hello");
   * });
   *
   * Deno.bench(async function myOtherTestName(): Promise<void> {
   *   const decoder = new TextDecoder("utf-8");
   *   const data = await Deno.readFile("hello_world.txt");
   *   assertEquals(decoder.decode(data), "Hello world");
   * });
   * ```
   *
   * @category Testing
   */
  export function bench(fn: () => void | Promise<void>): void;

  /** Register a bench which will be run when `deno bench` is used on the command
   * line and the containing module looks like a bench module.
   * `fn` can be async if required.
   *
   * ```ts
   * import {assert, fail, assertEquals} from "https://deno.land/std/testing/asserts.ts";
   *
   * Deno.bench("My test description", { permissions: { read: true } }, (): void => {
   *   assertEquals("hello", "hello");
   * });
   *
   * Deno.bench("My async test description", { permissions: { read: false } }, async (): Promise<void> => {
   *   const decoder = new TextDecoder("utf-8");
   *   const data = await Deno.readFile("hello_world.txt");
   *   assertEquals(decoder.decode(data), "Hello world");
   * });
   * ```
   *
   * @category Testing
   */
  export function bench(
    name: string,
    options: Omit<BenchDefinition, "fn" | "name">,
    fn: () => void | Promise<void>,
  ): void;

  /** Register a bench which will be run when `deno bench` is used on the command
   * line and the containing module looks like a bench module.
   * `fn` can be async if required.
   *
   * ```ts
   * import {assert, fail, assertEquals} from "https://deno.land/std/testing/asserts.ts";
   *
   * Deno.bench({ name: "My test description", permissions: { read: true } }, (): void => {
   *   assertEquals("hello", "hello");
   * });
   *
   * Deno.bench({ name: "My async test description", permissions: { read: false } }, async (): Promise<void> => {
   *   const decoder = new TextDecoder("utf-8");
   *   const data = await Deno.readFile("hello_world.txt");
   *   assertEquals(decoder.decode(data), "Hello world");
   * });
   * ```
   *
   * @category Testing
   */
  export function bench(
    options: Omit<BenchDefinition, "fn">,
    fn: () => void | Promise<void>,
  ): void;

  /** Register a bench which will be run when `deno bench` is used on the command
   * line and the containing module looks like a bench module.
   * `fn` can be async if required. Declared function must have a name.
   *
   * ```ts
   * import {assert, fail, assertEquals} from "https://deno.land/std/testing/asserts.ts";
   *
   * Deno.bench({ permissions: { read: true } }, function myTestName(): void {
   *   assertEquals("hello", "hello");
   * });
   *
   * Deno.bench({ permissions: { read: false } }, async function myOtherTestName(): Promise<void> {
   *   const decoder = new TextDecoder("utf-8");
   *   const data = await Deno.readFile("hello_world.txt");
   *   assertEquals(decoder.decode(data), "Hello world");
   * });
   * ```
   *
   * @category Testing
   */
  export function bench(
    options: Omit<BenchDefinition, "fn" | "name">,
    fn: () => void | Promise<void>,
  ): void;

  /**
   * **UNSTABLE**: New API, yet to be vetted.  This API is under consideration to
   * determine if permissions are required to call it.
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
   * NOTE:  This API is not implemented on Windows
   *
   * @category File System
   */
  export function umask(mask?: number): number;

  /** **UNSTABLE**: New API, yet to be vetted.
   *
   * Gets the size of the console as columns/rows.
   *
   * ```ts
   * const { columns, rows } = Deno.consoleSize(Deno.stdout.rid);
   * ```
   *
   * @category I/O
   */
  export function consoleSize(
    rid: number,
  ): {
    columns: number;
    rows: number;
  };

  /** **Unstable**  There are questions around which permission this needs. And
   * maybe should be renamed (loadAverage?)
   *
   * Returns an array containing the 1, 5, and 15 minute load averages. The
   * load average is a measure of CPU and IO utilization of the last one, five,
   * and 15 minute periods expressed as a fractional number.  Zero means there
   * is no load. On Windows, the three values are always the same and represent
   * the current load, not the 1, 5 and 15 minute load averages.
   *
   * ```ts
   * console.log(Deno.loadavg());  // e.g. [ 0.71, 0.44, 0.44 ]
   * ```
   *
   * Requires `allow-env` permission.
   *
   * @tags allow-env
   * @category Observability
   */
  export function loadavg(): number[];

  /** **Unstable** new API. yet to be vetted. Under consideration to possibly move to
   * Deno.build or Deno.versions and if it should depend sys-info, which may not
   * be desireable.
   *
   * Returns the release version of the Operating System.
   *
   * ```ts
   * console.log(Deno.osRelease());
   * ```
   *
   * Requires `allow-env` permission.
   *
   * @tags allow-env
   * @category Runtime Environment
   */
  export function osRelease(): string;

  /** **Unstable** new API. yet to be vetted.
   *
   * Displays the total amount of free and used physical and swap memory in the
   * system, as well as the buffers and caches used by the kernel.
   *
   * This is similar to the `free` command in Linux
   *
   * ```ts
   * console.log(Deno.systemMemoryInfo());
   * ```
   *
   * Requires `allow-env` permission.
   *
   * @tags allow-env
   * @category Runtime Environment
   */
  export function systemMemoryInfo(): SystemMemoryInfo;

  /** @category Runtime Environment */
  export interface SystemMemoryInfo {
    /** Total installed memory */
    total: number;
    /** Unused memory */
    free: number;
    /** Estimation of how much memory is available  for  starting  new
     * applications, without  swapping. Unlike the data provided by the cache or
     * free fields, this field takes into account page cache and also that not
     * all reclaimable memory slabs will be reclaimed due to items being in use
     */
    available: number;
    /** Memory used by kernel buffers */
    buffers: number;
    /** Memory  used  by  the  page  cache  and  slabs */
    cached: number;
    /** Total swap memory */
    swapTotal: number;
    /** Unused swap memory */
    swapFree: number;
  }

  /** The information of the network interface.
   *
   * @category Network
   */
  export interface NetworkInterfaceInfo {
    /** The network interface name */
    name: string;
    /** The IP protocol version */
    family: "IPv4" | "IPv6";
    /** The IP address */
    address: string;
    /** The netmask */
    netmask: string;
    /** The IPv6 scope id or null */
    scopeid: number | null;
    /** The CIDR range */
    cidr: string;
    /** The MAC address */
    mac: string;
  }

  /** **Unstable** new API. yet to be vetted.
   *
   * Returns an array of the network interface informations.
   *
   * ```ts
   * console.log(Deno.networkInterfaces());
   * ```
   *
   * Requires `allow-env` permission.
   *
   * @tags allow-env
   * @category Network
   */
  export function networkInterfaces(): NetworkInterfaceInfo[];

  /** **Unstable** new API. yet to be vetted.
   *
   * Returns the user id of the process on POSIX platforms. Returns null on windows.
   *
   * ```ts
   * console.log(Deno.getUid());
   * ```
   *
   * Requires `allow-env` permission.
   *
   * @tags allow-env
   * @category Runtime Environment
   */
  export function getUid(): number | null;

  /** **Unstable** new API. yet to be vetted.
   *
   * Returns the group id of the process on POSIX platforms. Returns null on windows.
   *
   * ```ts
   * console.log(Deno.getGid());
   * ```
   *
   * Requires `allow-env` permission.
   *
   * @tags allow-env
   * @category Runtime Environment
   */
  export function getGid(): number | null;

  /** All plain number types for interfacing with foreign functions.
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

  /** All BigInt number types for interfacing with foreign functions.
   *
   * @category FFI
   */
  type NativeBigIntType =
    | "u64"
    | "i64"
    | "usize"
    | "isize";

  /** @category FFI */
  type NativeBooleanType = "bool";

  /** @category FFI */
  type NativePointerType = "pointer";

  /** @category FFI */
  type NativeBufferType = "buffer";

  /** @category FFI */
  type NativeFunctionType = "function";

  /** @category FFI */
  type NativeVoidType = "void";

  /** All possible types for interfacing with foreign functions.
   *
   * @category FFI
   */
  export type NativeType =
    | NativeNumberType
    | NativeBigIntType
    | NativeBooleanType
    | NativePointerType
    | NativeBufferType
    | NativeFunctionType;

  /** @category FFI */
  export type NativeResultType = NativeType | NativeVoidType;

  /** @category FFI */
  type ToNativeTypeMap =
    & Record<NativeNumberType, number>
    & Record<NativeBigIntType, PointerValue>
    & Record<NativeBooleanType, boolean>
    & Record<NativePointerType, PointerValue | null>
    & Record<NativeFunctionType, PointerValue | null>
    & Record<NativeBufferType, TypedArray | null>;

  /** Type conversion for foreign symbol parameters and unsafe callback return
   * types.
   *
   * @category FFI
   */
  type ToNativeType<T extends NativeType = NativeType> = ToNativeTypeMap[T];

  /** @category FFI */
  type ToNativeResultTypeMap = ToNativeTypeMap & Record<NativeVoidType, void>;

  /** Type conversion for unsafe callback return types.
   *
   * @category FFI
   */
  type ToNativeResultType<T extends NativeResultType = NativeResultType> =
    ToNativeResultTypeMap[T];

  /** @category FFI */
  type ToNativeParameterTypes<T extends readonly NativeType[]> =
    //
    [(T[number])[]] extends [T] ? ToNativeType<T[number]>[]
      : [readonly (T[number])[]] extends [T]
        ? readonly ToNativeType<T[number]>[]
      : T extends readonly [...NativeType[]] ? {
          [K in keyof T]: ToNativeType<T[K]>;
        }
      : never;

  /** @category FFI */
  type FromNativeTypeMap =
    & Record<NativeNumberType, number>
    & Record<NativeBigIntType, PointerValue>
    & Record<NativeBooleanType, boolean>
    & Record<NativePointerType, PointerValue>
    & Record<NativeBufferType, PointerValue>
    & Record<NativeFunctionType, PointerValue>;

  /** Type conversion for foreign symbol return types and unsafe callback
   * parameters.
   *
   * @category FFI
   */
  type FromNativeType<T extends NativeType = NativeType> = FromNativeTypeMap[T];

  /** @category FFI */
  type FromNativeResultTypeMap =
    & FromNativeTypeMap
    & Record<NativeVoidType, void>;

  /** Type conversion for foreign symbol return types.
   *
   * @category FFI
   */
  type FromNativeResultType<T extends NativeResultType = NativeResultType> =
    FromNativeResultTypeMap[T];

  /** @category FFI */
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

  /** A foreign function as defined by its parameter and result types.
   *
   * @category FFI
   */
  export interface ForeignFunction<
    Parameters extends readonly NativeType[] = readonly NativeType[],
    Result extends NativeResultType = NativeResultType,
    NonBlocking extends boolean = boolean,
  > {
    /** Name of the symbol, defaults to the key name in symbols object. */
    name?: string;
    parameters: Parameters;
    result: Result;
    /** When true, function calls will run on a dedicated blocking thread and will return a Promise resolving to the `result`. */
    nonblocking?: NonBlocking;
    /** When true, function calls can safely callback into JS or trigger a GC event. Default is `false`. */
    callback?: boolean;
  }

  /** @category FFI */
  export interface ForeignStatic<Type extends NativeType = NativeType> {
    /** Name of the symbol, defaults to the key name in symbols object. */
    name?: string;
    type: Type;
  }

  /** A foreign library interface descriptor.
   *
   * @category FFI
   */
  export interface ForeignLibraryInterface {
    [name: string]: ForeignFunction | ForeignStatic;
  }

  /** Infers a foreign symbol.
   *
   * @category FFI
   */
  type StaticForeignSymbol<T extends ForeignFunction | ForeignStatic> =
    T extends ForeignFunction ? FromForeignFunction<T>
      : T extends ForeignStatic ? FromNativeType<T["type"]>
      : never;

  /** @category FFI */
  type FromForeignFunction<T extends ForeignFunction> = T["parameters"] extends
    readonly [] ? () => StaticForeignSymbolReturnType<T>
    : (
      ...args: ToNativeParameterTypes<T["parameters"]>
    ) => StaticForeignSymbolReturnType<T>;

  /** @category FFI */
  type StaticForeignSymbolReturnType<T extends ForeignFunction> =
    ConditionalAsync<T["nonblocking"], FromNativeResultType<T["result"]>>;

  /** @category FFI */
  type ConditionalAsync<IsAsync extends boolean | undefined, T> =
    IsAsync extends true ? Promise<T> : T;

  /** Infers a foreign library interface.
   *
   * @category FFI
   */
  type StaticForeignLibraryInterface<T extends ForeignLibraryInterface> = {
    [K in keyof T]: StaticForeignSymbol<T[K]>;
  };

  /** @category FFI */
  type TypedArray =
    | Int8Array
    | Uint8Array
    | Int16Array
    | Uint16Array
    | Int32Array
    | Uint32Array
    | Uint8ClampedArray
    | Float32Array
    | Float64Array
    | BigInt64Array
    | BigUint64Array;

  /**
   * Pointer type depends on the architecture and actual pointer value.
   *
   * On a 32 bit system all pointer values are plain numbers. On a 64 bit
   * system pointer values are represented as numbers if the value is below
   * `Number.MAX_SAFE_INTEGER`.
   *
   * @category FFI
   */
  export type PointerValue = number | bigint;

  /** **UNSTABLE**: Unsafe and new API, beware!
   *
   * An unsafe pointer to a memory location for passing and returning pointers
   * to and from the FFI.
   *
   * @category FFI
   */
  export class UnsafePointer {
    /**
     * Return the direct memory pointer to the typed array in memory
     */
    static of(value: Deno.UnsafeCallback | TypedArray): PointerValue;
  }

  /** **UNSTABLE**: Unsafe and new API, beware!
   *
   * An unsafe pointer view to a memory location as specified by the `pointer`
   * value. The `UnsafePointerView` API mimics the standard built in interface
   * `DataView` for accessing the underlying types at an memory location
   * (numbers, strings and raw bytes).
   *
   * @category FFI
   */
  export class UnsafePointerView {
    constructor(pointer: bigint);

    pointer: bigint;

    /** Gets a boolean at the specified byte offset from the pointer. */
    getBool(offset?: number): boolean;
    /** Gets an unsigned 8-bit integer at the specified byte offset from the pointer. */
    getUint8(offset?: number): number;
    /** Gets a signed 8-bit integer at the specified byte offset from the pointer. */
    getInt8(offset?: number): number;
    /** Gets an unsigned 16-bit integer at the specified byte offset from the pointer. */
    getUint16(offset?: number): number;
    /** Gets a signed 16-bit integer at the specified byte offset from the pointer. */
    getInt16(offset?: number): number;
    /** Gets an unsigned 32-bit integer at the specified byte offset from the pointer. */
    getUint32(offset?: number): number;
    /** Gets a signed 32-bit integer at the specified byte offset from the pointer. */
    getInt32(offset?: number): number;
    /** Gets an unsigned 64-bit integer at the specified byte offset from the pointer. */
    getBigUint64(offset?: number): PointerValue;
    /** Gets a signed 64-bit integer at the specified byte offset from the pointer. */
    getBigInt64(offset?: number): PointerValue;
    /** Gets a signed 32-bit float at the specified byte offset from the pointer. */
    getFloat32(offset?: number): number;
    /** Gets a signed 64-bit float at the specified byte offset from the pointer. */
    getFloat64(offset?: number): number;
    /** Gets a C string (null terminated string) at the specified byte offset from the pointer. */
    getCString(offset?: number): string;
    /** Gets a C string (null terminated string) at the specified byte offset from the specified pointer. */
    static getCString(pointer: PointerValue, offset?: number): string;
    /** Gets an ArrayBuffer of length `byteLength` at the specified byte offset from the pointer. */
    getArrayBuffer(byteLength: number, offset?: number): ArrayBuffer;
    /** Gets an ArrayBuffer of length `byteLength` at the specified byte offset from the specified pointer. */
    static getArrayBuffer(
      pointer: PointerValue,
      byteLength: number,
      offset?: number,
    ): ArrayBuffer;
    /** Copies the memory of the pointer into a typed array. Length is determined from the typed array's `byteLength`. Also takes optional byte offset from the pointer. */
    copyInto(destination: TypedArray, offset?: number): void;
    /** Copies the memory of the specified pointer into a typed array. Length is determined from the typed array's `byteLength`. Also takes optional byte offset from the pointer. */
    static copyInto(
      pointer: PointerValue,
      destination: TypedArray,
      offset?: number,
    ): void;
  }

  /**
   * **UNSTABLE**: Unsafe and new API, beware!
   *
   * An unsafe pointer to a function, for calling functions that are not
   * present as symbols.
   *
   * @category FFI
   */
  export class UnsafeFnPointer<Fn extends ForeignFunction> {
    pointer: bigint;
    definition: Fn;

    constructor(pointer: bigint, definition: Fn);

    call: FromForeignFunction<Fn>;
  }

  /** @category FFI */
  export interface UnsafeCallbackDefinition<
    Parameters extends readonly NativeType[] = readonly NativeType[],
    Result extends NativeResultType = NativeResultType,
  > {
    parameters: Parameters;
    result: Result;
  }

  /** @category FFI */
  type UnsafeCallbackFunction<
    Parameters extends readonly NativeType[] = readonly NativeType[],
    Result extends NativeResultType = NativeResultType,
  > = Parameters extends readonly [] ? () => ToNativeResultType<Result> : (
    ...args: FromNativeParameterTypes<Parameters>
  ) => ToNativeResultType<Result>;

  /**
   * **UNSTABLE**: Unsafe and new API, beware!
   *
   * An unsafe function pointer for passing JavaScript functions
   * as C function pointers to ffi calls.
   *
   * The function pointer remains valid until the `close()` method is called.
   *
   * The callback can be explicitly ref'ed and deref'ed to stop Deno's
   * process from exiting.
   *
   * @category FFI
   */
  export class UnsafeCallback<
    Definition extends UnsafeCallbackDefinition = UnsafeCallbackDefinition,
  > {
    constructor(
      definition: Definition,
      callback: UnsafeCallbackFunction<
        Definition["parameters"],
        Definition["result"]
      >,
    );

    pointer: bigint;
    definition: Definition;
    callback: UnsafeCallbackFunction<
      Definition["parameters"],
      Definition["result"]
    >;

    /**
     * Adds one to this callback's reference counting.
     *
     * If the callback's reference count becomes non-zero, it will keep
     * Deno's process from exiting.
     */
    ref(): void;

    /**
     * Removes one from this callback's reference counting.
     *
     * If the callback's reference counter becomes zero, it will no longer
     * keep Deno's process from exiting.
     */
    unref(): void;

    /**
     * Removes the C function pointer associated with the UnsafeCallback.
     * Continuing to use the instance after calling this object will lead to errors
     * and crashes.
     *
     * Calling this method will also immediately set the callback's reference
     * counting to zero and it will no longer keep Deno's process from exiting.
     */
    close(): void;
  }

  /**
   * A dynamic library resource
   *
   * @category FFI
   */
  export interface DynamicLibrary<S extends ForeignLibraryInterface> {
    /** All of the registered library along with functions for calling them */
    symbols: StaticForeignLibraryInterface<S>;
    close(): void;
  }

  /** **UNSTABLE**: Unsafe and new API, beware!
   *
   * Opens a dynamic library and registers symbols
   *
   * @category FFI
   */
  export function dlopen<S extends ForeignLibraryInterface>(
    filename: string | URL,
    symbols: S,
  ): DynamicLibrary<S>;

  /** @category I/O */
  export type SetRawOptions = {
    cbreak: boolean;
  };

  /** **UNSTABLE**: new API, yet to be vetted
   *
   * Set TTY to be under raw mode or not. In raw mode, characters are read and
   * returned as is, without being processed. All special processing of
   * characters by the terminal is disabled, including echoing input characters.
   * Reading from a TTY device in raw mode is faster than reading from a TTY
   * device in canonical mode.
   *
   * The `cbreak` option can be used to indicate that characters that correspond
   * to a signal should still be generated. When disabling raw mode, this option
   * is ignored. This functionality currently only works on Linux and Mac OS.
   *
   * ```ts
   * Deno.setRaw(Deno.stdin.rid, true, { cbreak: true });
   * ```
   *
   * @category I/O
   */
  export function setRaw(
    rid: number,
    mode: boolean,
    options?: SetRawOptions,
  ): void;

  /** **UNSTABLE**: needs investigation into high precision time.
   *
   * Synchronously changes the access (`atime`) and modification (`mtime`) times
   * of a file system object referenced by `path`. Given times are either in
   * seconds (UNIX epoch time) or as `Date` objects.
   *
   * ```ts
   * Deno.utimeSync("myfile.txt", 1556495550, new Date());
   * ```
   *
   * Requires `allow-write` permission.
   *
   * @tags allow-write
   * @category File System
   */
  export function utimeSync(
    path: string | URL,
    atime: number | Date,
    mtime: number | Date,
  ): void;

  /** **UNSTABLE**: needs investigation into high precision time.
   *
   * Changes the access (`atime`) and modification (`mtime`) times of a file
   * system object referenced by `path`. Given times are either in seconds
   * (UNIX epoch time) or as `Date` objects.
   *
   * ```ts
   * await Deno.utime("myfile.txt", 1556495550, new Date());
   * ```
   *
   * Requires `allow-write` permission.
   *
   * @tags allow-write
   * @category File System
   */
  export function utime(
    path: string | URL,
    atime: number | Date,
    mtime: number | Date,
  ): Promise<void>;

  /** @category Sub Process */
  export function run<
    T extends RunOptions & {
      clearEnv?: boolean;
      gid?: number;
      uid?: number;
    } = RunOptions & {
      clearEnv?: boolean;
      gid?: number;
      uid?: number;
    },
  >(opt: T): Process<T>;

  /**  **UNSTABLE**: New API, yet to be vetted.  Additional consideration is still
   * necessary around the permissions required.
   *
   * Get the `hostname` of the machine the Deno process is running on.
   *
   * ```ts
   * console.log(Deno.hostname());
   * ```
   *
   *  Requires `allow-env` permission.
   *
   * @tags allow-env
   * @category Runtime Environment
   */
  export function hostname(): string;

  /** **UNSTABLE**: New API, yet to be vetted.
   * A custom HttpClient for use with `fetch`.
   *
   * ```ts
   * const caCert = await Deno.readTextFile("./ca.pem");
   * const client = Deno.createHttpClient({ caCerts: [ caCert ] });
   * const req = await fetch("https://myserver.com", { client });
   * ```
   *
   * @category Fetch API
   */
  export class HttpClient {
    rid: number;
    close(): void;
  }

  /** **UNSTABLE**: New API, yet to be vetted.
   * The options used when creating a [HttpClient].
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
  }

  /** @category Fetch API */
  export interface Proxy {
    url: string;
    basicAuth?: BasicAuth;
  }

  /** @category Fetch API */
  export interface BasicAuth {
    username: string;
    password: string;
  }

  /** **UNSTABLE**: New API, yet to be vetted.
   * Create a custom HttpClient for to use with `fetch`.
   *
   * ```ts
   * const caCert = await Deno.readTextFile("./ca.pem");
   * const client = Deno.createHttpClient({ caCerts: [ caCert ] });
   * const response = await fetch("https://myserver.com", { client });
   * ```
   *
   * ```ts
   * const client = Deno.createHttpClient({ proxy: { url: "http://myproxy.com:8080" } });
   * const response = await fetch("https://myserver.com", { client });
   * ```
   *
   * @category Fetch API
   */
  export function createHttpClient(
    options: CreateHttpClientOptions,
  ): HttpClient;

  /** **UNSTABLE**: needs investigation into high precision time.
   *
   * Synchronously changes the access (`atime`) and modification (`mtime`) times
   * of a file stream resource referenced by `rid`. Given times are either in
   * seconds (UNIX epoch time) or as `Date` objects.
   *
   * ```ts
   * const file = Deno.openSync("file.txt", { create: true, write: true });
   * Deno.futimeSync(file.rid, 1556495550, new Date());
   * ```
   *
   * @category File System
   */
  export function futimeSync(
    rid: number,
    atime: number | Date,
    mtime: number | Date,
  ): void;

  /** **UNSTABLE**: needs investigation into high precision time.
   *
   * Changes the access (`atime`) and modification (`mtime`) times of a file
   * stream resource referenced by `rid`. Given times are either in seconds
   * (UNIX epoch time) or as `Date` objects.
   *
   * ```ts
   * const file = await Deno.open("file.txt", { create: true, write: true });
   * await Deno.futime(file.rid, 1556495550, new Date());
   * ```
   *
   * @category File System
   */
  export function futime(
    rid: number,
    atime: number | Date,
    mtime: number | Date,
  ): Promise<void>;

  /** **UNSTABLE**: new API, yet to be vetted.
   *
   * A generic transport listener for message-oriented protocols.
   *
   * @category Network
   */
  export interface DatagramConn extends AsyncIterable<[Uint8Array, Addr]> {
    /** **UNSTABLE**: new API, yet to be vetted.
     *
     * Waits for and resolves to the next message to the `UDPConn`. */
    receive(p?: Uint8Array): Promise<[Uint8Array, Addr]>;
    /** UNSTABLE: new API, yet to be vetted.
     *
     * Sends a message to the target. */
    send(p: Uint8Array, addr: Addr): Promise<number>;
    /** UNSTABLE: new API, yet to be vetted.
     *
     * Close closes the socket. Any pending message promises will be rejected
     * with errors. */
    close(): void;
    /** Return the address of the `UDPConn`. */
    readonly addr: Addr;
    [Symbol.asyncIterator](): AsyncIterableIterator<[Uint8Array, Addr]>;
  }

  /** @category Network */
  export interface UnixListenOptions {
    /** A Path to the Unix Socket. */
    path: string;
  }

  /** **UNSTABLE**: new API, yet to be vetted.
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

  /** **UNSTABLE**: new API, yet to be vetted
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
    options: ListenOptions & { transport: "udp" },
  ): DatagramConn;

  /** **UNSTABLE**: new API, yet to be vetted
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

  /** @category Network */
  export interface UnixConnectOptions {
    transport: "unix";
    path: string;
  }

  /** **UNSTABLE**:  The unix socket transport is unstable as a new API yet to
   * be vetted.  The TCP transport is considered stable.
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
  export function connect(
    options: ConnectOptions,
  ): Promise<TcpConn>;
  export function connect(
    options: UnixConnectOptions,
  ): Promise<UnixConn>;

  /** @category Network */
  export interface ConnectTlsOptions {
    /** PEM formatted client certificate chain. */
    certChain?: string;
    /** PEM formatted (RSA or PKCS8) private key of client certificate. */
    privateKey?: string;
    /** **UNSTABLE**: new API, yet to be vetted.
     *
     * Application-Layer Protocol Negotiation (ALPN) protocols supported by
     * the client. If not specified, no ALPN extension will be included in the
     * TLS handshake.
     */
    alpnProtocols?: string[];
  }

  /** @category Network */
  export interface TlsHandshakeInfo {
    /** **UNSTABLE**: new API, yet to be vetted.
     *
     * Contains the ALPN protocol selected during negotiation with the server.
     * If no ALPN protocol selected, returns `null`.
     */
    alpnProtocol: string | null;
  }

  /** @category Network */
  export interface TlsConn extends Conn {
    /** Runs the client or server handshake protocol to completion if that has
     * not happened yet. Calling this method is optional; the TLS handshake
     * will be completed automatically as soon as data is sent or received. */
    handshake(): Promise<TlsHandshakeInfo>;
  }

  /** **UNSTABLE** New API, yet to be vetted.
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

  /** @category Network */
  export interface ListenTlsOptions {
    /** **UNSTABLE**: new API, yet to be vetted.
     *
     * Application-Layer Protocol Negotiation (ALPN) protocols to announce to
     * the client. If not specified, no ALPN extension will be included in the
     * TLS handshake.
     */
    alpnProtocols?: string[];
  }

  /** @category Network */
  export interface StartTlsOptions {
    /** **UNSTABLE**: new API, yet to be vetted.
     *
     * Application-Layer Protocol Negotiation (ALPN) protocols to announce to
     * the client. If not specified, no ALPN extension will be included in the
     * TLS handshake.
     */
    alpnProtocols?: string[];
  }

  /** @category Network */
  export interface Listener extends AsyncIterable<Conn> {
    /** **UNSTABLE**: new API, yet to be vetted.
     *
     * Make the listener block the event loop from finishing.
     *
     * Note: the listener blocks the event loop from finishing by default.
     * This method is only meaningful after `.unref()` is called.
     */
    ref(): void;
    /** **UNSTABLE**: new API, yet to be vetted.
     *
     * Make the listener not block the event loop from finishing.
     */
    unref(): void;
  }

  /** **UNSTABLE**: New API should be tested first.
   *
   * Acquire an advisory file-system lock for the provided file. `exclusive`
   * defaults to `false`.
   *
   * @category File System
   */
  export function flock(rid: number, exclusive?: boolean): Promise<void>;

  /** **UNSTABLE**: New API should be tested first.
   *
   * Acquire an advisory file-system lock for the provided file. `exclusive`
   * defaults to `false`.
   *
   * @category File System
   */
  export function flockSync(rid: number, exclusive?: boolean): void;

  /** **UNSTABLE**: New API should be tested first.
   *
   * Release an advisory file-system lock for the provided file.
   *
   * @category File System
   */
  export function funlock(rid: number): Promise<void>;

  /** **UNSTABLE**: New API should be tested first.
   *
   * Release an advisory file-system lock for the provided file.
   *
   * @category File System
   */
  export function funlockSync(rid: number): void;

  /** **UNSTABLE**: new API, yet to be vetted.
   *
   * Make the timer of the given id blocking the event loop from finishing.
   *
   * @category Timers
   */
  export function refTimer(id: number): void;

  /** **UNSTABLE**: new API, yet to be vetted.
   *
   * Make the timer of the given id not blocking the event loop from finishing.
   *
   * @category Timers
   */
  export function unrefTimer(id: number): void;

  /**
   * A handler for HTTP requests. Consumes a request and returns a response.
   *
   * If a handler throws, the server calling the handler will assume the impact
   * of the error is isolated to the individual request. It will catch the error
   * and if necessary will close the underlying connection.
   *
   * @category HTTP Server
   */
  export type ServeHandler = (request: Request) => Response | Promise<Response>;

  /**
   * @category HTTP Server
   */
  export interface ServeOptions extends Partial<Deno.ListenOptions> {
    /** An AbortSignal to close the server and all connections. */
    signal?: AbortSignal;

    /** The handler to invoke when route handlers throw an error. */
    onError?: (error: unknown) => Response | Promise<Response>;

    /** The callback which is called when the server started listening */
    onListen?: (params: { hostname: string; port: number }) => void;
  }

  /**
   * @category HTTP Server
   */
  export interface ServeTlsOptions extends ServeOptions {
    /** Server private key in PEM format */
    cert: string;

    /** Cert chain in PEM format */
    key: string;
  }

  /**
   * @category HTTP Server
   */
  export interface ServeInit {
    /** The handler to invoke to process each incoming request. */
    handler: ServeHandler;
  }

  /** **UNSTABLE**: new API, yet to be vetted.
   *
   * Serves HTTP requests with the given handler.
   *
   * You can specify an object with a port and hostname option, which is the
   * address to listen on. The default is port 9000 on hostname "127.0.0.1".
   *
   * The below example serves with the port 9000.
   *
   * ```ts
   * Deno.serve((_req) => new Response("Hello, world"));
   * ```
   *
   * You can change the address to listen on using the `hostname` and `port`
   * options. The below example serves on port 3000.
   *
   * ```ts
   * Deno.serve({ port: 3000 }, (_req) => new Response("Hello, world"));
   * ```
   *
   * You can stop the server with an AbortSignal. The abort signal needs to be
   * passed as the `signal` option in the options bag. The server aborts when
   * the abort signal is aborted. To wait for the server to close, await the
   * promise returned from the `Deno.serve` API.
   *
   * ```ts
   * const ac = new AbortController();
   *
   * Deno.serve({ signal: ac.signal }, (_req) => new Response("Hello, world"))
   *  .then(() => console.log("Server closed"));
   *
   * console.log("Closing server...");
   * ac.abort();
   * ```
   *
   * By default `Deno.serve` prints the message `Listening on http://<hostname>:<port>/`
   * on start up. If you like to change this behaviour, you can specify a custom
   * `onListen` callback.
   *
   * ```ts
   * Deno.serve({
   *   onListen({ port, hostname }) {
   *     console.log(`Server started at http://${hostname}:${port}`);
   *     // ... more info specific to your server ..
   *   },
   *   handler: (_req) => new Response("Hello, world"),
   * });
   * ```
   *
   * To enable TLS you must specify the `key` and `cert` options.
   *
   * ```ts
   * const cert = "-----BEGIN CERTIFICATE-----\n...\n-----END CERTIFICATE-----\n";
   * const key = "-----BEGIN PRIVATE KEY-----\n...\n-----END PRIVATE KEY-----\n";
   * Deno.serve({ cert, key }, (_req) => new Response("Hello, world"));
   * ```
   *
   * @category HTTP Server
   */
  export function serve(
    handler: ServeHandler,
    options?: ServeOptions | ServeTlsOptions,
  ): Promise<void>;
  export function serve(
    options: ServeOptions | ServeTlsOptions,
    handler: ServeHandler,
  ): Promise<void>;
  export function serve(
    options: ServeInit & (ServeOptions | ServeTlsOptions),
  ): Promise<void>;

  /** **UNSTABLE**: new API, yet to be vetted.
   *
   * Allows "hijacking" the connection that the request is associated with.
   * This can be used to implement protocols that build on top of HTTP (eg.
   * WebSockets).
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
   * This method can only be called on requests originating the `Deno.serveHttp`
   * server.
   *
   * @category HTTP Server
   */
  export function upgradeHttp(
    request: Request,
  ): Promise<[Deno.Conn, Uint8Array]>;

  /** **UNSTABLE**: new API, yet to be vetted.
   *
   * Allows "hijacking" the connection that the request is associated with.
   * This can be used to implement protocols that build on top of HTTP (eg.
   * WebSockets).

   * Unlike `Deno.upgradeHttp` this function does not require that you respond
   * to the request with a `Response` object. Instead this function returns
   * the underlying connection and first packet received immediately, and then
   * the caller is responsible for writing the response to the connection.
   *
   * This method can only be called on requests originating the `Deno.serve`
   * server.
   *
   * @category HTTP Server
   */
  export function upgradeHttpRaw(request: Request): [Deno.Conn, Uint8Array];

  /** @category Sub Process */
  export interface SpawnOptions {
    /** Arguments to pass to the process. */
    args?: string[];
    /**
     * The working directory of the process.
     * If not specified, the cwd of the parent process is used.
     */
    cwd?: string | URL;
    /**
     * Clear environmental variables from parent process.
     * Doesn't guarantee that only `opt.env` variables are present,
     * as the OS may set environmental variables for processes.
     */
    clearEnv?: boolean;
    /** Environmental variables to pass to the subprocess. */
    env?: Record<string, string>;
    /**
     * Sets the child process’s user ID. This translates to a setuid call
     * in the child process. Failure in the setuid call will cause the spawn to fail.
     */
    uid?: number;
    /** Similar to `uid`, but sets the group ID of the child process. */
    gid?: number;
    /**
     * An AbortSignal that allows closing the process using the corresponding
     * AbortController by sending the process a SIGTERM signal.
     * Not Supported by execSync.
     */
    signal?: AbortSignal;

    /** Defaults to "null". */
    stdin?: "piped" | "inherit" | "null";
    /** Defaults to "piped". */
    stdout?: "piped" | "inherit" | "null";
    /** Defaults to "piped". */
    stderr?: "piped" | "inherit" | "null";
  }

  /**
   * Spawns a child process.
   *
   * If any stdio options are not set to `"piped"`, accessing the corresponding
   * field on the `Child` or its `SpawnOutput` will throw a `TypeError`.
   *
   * If stdin is set to `"piped"`, the stdin WritableStream needs to be closed
   * manually.
   *
   * ```ts
   * const child = Deno.spawnChild(Deno.execPath(), {
   *   args: [
   *     "eval",
   *     "console.log('Hello World')",
   *   ],
   *   stdin: "piped",
   * });
   *
   * // open a file and pipe the subprocess output to it.
   * child.stdout.pipeTo(Deno.openSync("output").writable);
   *
   * // manually close stdin
   * child.stdin.close();
   * const status = await child.status;
   * ```
   *
   * @category Sub Process
   */
  export function spawnChild(
    command: string | URL,
    options?: SpawnOptions,
  ): Child;

  /** @category Sub Process */
  export class Child {
    get stdin(): WritableStream<Uint8Array>;
    get stdout(): ReadableStream<Uint8Array>;
    get stderr(): ReadableStream<Uint8Array>;
    readonly pid: number;
    /** Get the status of the child. */
    readonly status: Promise<ChildStatus>;

    /** Waits for the child to exit completely, returning all its output and status. */
    output(): Promise<SpawnOutput>;
    /** Kills the process with given Signal. Defaults to SIGTERM. */
    kill(signo?: Signal): void;

    ref(): void;
    unref(): void;
  }

  /**
   * Executes a subprocess, waiting for it to finish and
   * collecting all of its output.
   * Will throw an error if `stdin: "piped"` is passed.
   *
   * If options `stdout` or `stderr` are not set to `"piped"`, accessing the
   * corresponding field on `SpawnOutput` will throw a `TypeError`.
   *
   * ```ts
   * const { code, stdout, stderr } = await Deno.spawn(Deno.execPath(), {
   *   args: [
   *     "eval",
   *        "console.log('hello'); console.error('world')",
   *   ],
   * });
   * console.assert(code === 0);
   * console.assert("hello\n" === new TextDecoder().decode(stdout));
   * console.assert("world\n" === new TextDecoder().decode(stderr));
   * ```
   *
   * @category Sub Process
   */
  export function spawn(
    command: string | URL,
    options?: SpawnOptions,
  ): Promise<SpawnOutput>;

  /**
   * Synchronously executes a subprocess, waiting for it to finish and
   * collecting all of its output.
   * Will throw an error if `stdin: "piped"` is passed.
   *
   * If options `stdout` or `stderr` are not set to `"piped"`, accessing the
   * corresponding field on `SpawnOutput` will throw a `TypeError`.
   *
   * ```ts
   * const { code, stdout, stderr } = Deno.spawnSync(Deno.execPath(), {
   *   args: [
   *     "eval",
   *       "console.log('hello'); console.error('world')",
   *   ],
   * });
   * console.assert(code === 0);
   * console.assert("hello\n" === new TextDecoder().decode(stdout));
   * console.assert("world\n" === new TextDecoder().decode(stderr));
   * ```
   *
   * @category Sub Process
   */
  export function spawnSync(
    command: string | URL,
    options?: SpawnOptions,
  ): SpawnOutput;

  /** @category Sub Process */
  export interface ChildStatus {
    success: boolean;
    code: number;
    signal: Signal | null;
  }

  /** @category Sub Process */
  export interface SpawnOutput extends ChildStatus {
    get stdout(): Uint8Array;
    get stderr(): Uint8Array;
  }
}

/**
 * @tags allow-net, allow-read
 * @category Fetch API
 */
declare function fetch(
  input: Request | URL | string,
  init?: RequestInit & { client: Deno.HttpClient },
): Promise<Response>;

/** @category Web Workers */
declare interface WorkerOptions {
  /** UNSTABLE: New API.
   *
   * Configure permissions options to change the level of access the worker will
   * have. By default it will have no permissions. Note that the permissions
   * of a worker can't be extended beyond its parent's permissions reach.
   * - "inherit" will take the permissions of the thread the worker is created in
   * - "none" will use the default behavior and have no permission
   * - You can provide a list of routes relative to the file the worker
   *   is created in to limit the access of the worker (read/write permissions only)
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

/** @category Web Sockets */
declare interface WebSocketStreamOptions {
  protocols?: string[];
  signal?: AbortSignal;
  headers?: HeadersInit;
}

/** @category Web Sockets */
declare interface WebSocketConnection {
  readable: ReadableStream<string | Uint8Array>;
  writable: WritableStream<string | Uint8Array>;
  extensions: string;
  protocol: string;
}

/** @category Web Sockets */
declare interface WebSocketCloseInfo {
  code?: number;
  reason?: string;
}

/**
 * @tags allow-net
 * @category Web Sockets
 */
declare class WebSocketStream {
  constructor(url: string, options?: WebSocketStreamOptions);
  url: string;
  connection: Promise<WebSocketConnection>;
  closed: Promise<WebSocketCloseInfo>;
  close(closeInfo?: WebSocketCloseInfo): void;
}
