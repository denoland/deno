// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

/// <reference no-default-lib="true" />
/// <reference lib="deno.ns" />

declare namespace Deno {
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
   */
  export function umask(mask?: number): number;

  /** **UNSTABLE**: New API, yet to be vetted.
   *
   * Gets the size of the console as columns/rows.
   *
   * ```ts
   * const { columns, rows } = Deno.consoleSize(Deno.stdout.rid);
   * ```
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
   */
  export function systemMemoryInfo(): SystemMemoryInfo;

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

  /** The information of the network interface */
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
   */
  export function getUid(): number | null;

  /** All possible types for interfacing with foreign functions */
  export type NativeType =
    | "void"
    | "u8"
    | "i8"
    | "u16"
    | "i16"
    | "u32"
    | "i32"
    | "u64"
    | "i64"
    | "usize"
    | "isize"
    | "f32"
    | "f64"
    | "pointer";

  /** A foreign function as defined by its parameter and result types */
  export interface ForeignFunction<
    Parameters extends readonly NativeType[] = readonly NativeType[],
    Result extends NativeType = NativeType,
    NonBlocking extends boolean = boolean,
  > {
    /** Name of the symbol, defaults to the key name in symbols object. */
    name?: string;
    parameters: Parameters;
    result: Result;
    /** When true, function calls will run on a dedicated blocking thread and will return a Promise resolving to the `result`. */
    nonblocking?: NonBlocking;
  }

  export interface ForeignStatic<Type extends NativeType = NativeType> {
    /** Name of the symbol, defaults to the key name in symbols object. */
    name?: string;
    type: Exclude<Type, "void">;
  }

  /** A foreign library interface descriptor */
  export interface ForeignLibraryInterface {
    [name: string]: ForeignFunction | ForeignStatic;
  }

  /** All possible number types interfacing with foreign functions */
  type StaticNativeNumberType = Exclude<NativeType, "void" | "pointer">;

  /** Infers a foreign function return type */
  type StaticForeignFunctionResult<T extends NativeType> = T extends "void"
    ? void
    : T extends StaticNativeNumberType ? number
    : T extends "pointer" ? UnsafePointer
    : never;

  type StaticForeignFunctionParameter<T> = T extends "void" ? void
    : T extends StaticNativeNumberType ? number
    : T extends "pointer" ? Deno.UnsafePointer | Deno.TypedArray | null
    : unknown;

  /** Infers a foreign function parameter list. */
  type StaticForeignFunctionParameters<T extends readonly NativeType[]> = [
    ...{
      [K in keyof T]: StaticForeignFunctionParameter<T[K]>;
    },
  ];

  /** Infers a foreign symbol */
  type StaticForeignSymbol<T extends ForeignFunction | ForeignStatic> =
    T extends ForeignFunction ? (
      ...args: StaticForeignFunctionParameters<T["parameters"]>
    ) => ConditionalAsync<
      T["nonblocking"],
      StaticForeignFunctionResult<T["result"]>
    >
      : T extends ForeignStatic ? StaticForeignFunctionResult<T["type"]>
      : never;

  type ConditionalAsync<IsAsync extends boolean | undefined, T> =
    IsAsync extends true ? Promise<T> : T;

  /** Infers a foreign library interface */
  type StaticForeignLibraryInterface<T extends ForeignLibraryInterface> = {
    [K in keyof T]: StaticForeignSymbol<T[K]>;
  };

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

  /** **UNSTABLE**: Unsafe and new API, beware!
   *
   * An unsafe pointer to a memory location for passing and returning pointers to and from the ffi
   */
  export class UnsafePointer {
    constructor(value: bigint);

    value: bigint;

    /**
     * Return the direct memory pointer to the typed array in memory
     */
    static of(typedArray: TypedArray): UnsafePointer;

    /**
     * Returns the value of the pointer which is useful in certain scenarios.
     */
    valueOf(): bigint;
  }

  /** **UNSTABLE**: Unsafe and new API, beware!
   *
   * An unsafe pointer view to a memory location as specified by the `pointer`
   * value. The `UnsafePointerView` API mimics the standard built in interface
   * `DataView` for accessing the underlying types at an memory location
   * (numbers, strings and raw bytes).
   */
  export class UnsafePointerView {
    constructor(pointer: UnsafePointer);

    pointer: UnsafePointer;

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
    getBigUint64(offset?: number): bigint;
    /** Gets a signed 64-bit integer at the specified byte offset from the pointer. */
    getBigInt64(offset?: number): bigint;
    /** Gets a signed 32-bit float at the specified byte offset from the pointer. */
    getFloat32(offset?: number): number;
    /** Gets a signed 64-bit float at the specified byte offset from the pointer. */
    getFloat64(offset?: number): number;
    /** Gets a C string (null terminated string) at the specified byte offset from the pointer. */
    getCString(offset?: number): string;
    /** Gets an ArrayBuffer of length `byteLength` at the specified byte offset from the pointer. */
    getArrayBuffer(byteLength: number, offset?: number): ArrayBuffer;
    /** Copies the memory of the pointer into a typed array. Length is determined from the typed array's `byteLength`. Also takes optional offset from the pointer. */
    copyInto(destination: TypedArray, offset?: number): void;
  }

  /**
   * **UNSTABLE**: Unsafe and new API, beware!
   *
   * An unsafe pointer to a function, for calling functions that are not
   * present as symbols.
   */
  export class UnsafeFnPointer<Fn extends ForeignFunction> {
    pointer: UnsafePointer;
    definition: Fn;

    constructor(pointer: UnsafePointer, definition: Fn);

    call(
      ...args: StaticForeignFunctionParameters<Fn["parameters"]>
    ): ConditionalAsync<
      Fn["nonblocking"],
      StaticForeignFunctionResult<Fn["result"]>
    >;
  }

  /** A dynamic library resource */
  export interface DynamicLibrary<S extends ForeignLibraryInterface> {
    /** All of the registered library along with functions for calling them */
    symbols: StaticForeignLibraryInterface<S>;
    close(): void;
  }

  /** **UNSTABLE**: Unsafe and new API, beware!
   *
   * Opens a dynamic library and registers symbols
   */
  export function dlopen<S extends ForeignLibraryInterface>(
    filename: string | URL,
    symbols: S,
  ): DynamicLibrary<S>;

  /** The log category for a diagnostic message. */
  export enum DiagnosticCategory {
    Warning = 0,
    Error = 1,
    Suggestion = 2,
    Message = 3,
  }

  export interface DiagnosticMessageChain {
    messageText: string;
    category: DiagnosticCategory;
    code: number;
    next?: DiagnosticMessageChain[];
  }

  export interface Diagnostic {
    /** A string message summarizing the diagnostic. */
    messageText?: string;
    /** An ordered array of further diagnostics. */
    messageChain?: DiagnosticMessageChain;
    /** Information related to the diagnostic. This is present when there is a
     * suggestion or other additional diagnostic information */
    relatedInformation?: Diagnostic[];
    /** The text of the source line related to the diagnostic. */
    sourceLine?: string;
    source?: string;
    /** The start position of the error. Zero based index. */
    start?: {
      line: number;
      character: number;
    };
    /** The end position of the error.  Zero based index. */
    end?: {
      line: number;
      character: number;
    };
    /** The filename of the resource related to the diagnostic message. */
    fileName?: string;
    /** The category of the diagnostic. */
    category: DiagnosticCategory;
    /** A number identifier. */
    code: number;
  }

  /** **UNSTABLE**: new API, yet to be vetted.
   *
   * Format an array of diagnostic items and return them as a single string in a
   * user friendly format. If there are no diagnostics then it will return an
   * empty string.
   *
   * ```ts
   * const { diagnostics } = await Deno.emit("file_with_compile_issues.ts");
   * console.table(diagnostics);  // Prints raw diagnostic data
   * console.log(Deno.formatDiagnostics(diagnostics));  // User friendly output of diagnostics
   * console.log(Deno.formatDiagnostics([]));  // An empty string
   * ```
   *
   * @param diagnostics An array of diagnostic items to format
   */
  export function formatDiagnostics(diagnostics: Diagnostic[]): string;

  /** **UNSTABLE**: new API, yet to be vetted.
   *
   * A specific subset TypeScript compiler options that can be supported by the
   * Deno TypeScript compiler. */
  export interface CompilerOptions {
    /** Allow JavaScript files to be compiled. Defaults to `true`. */
    allowJs?: boolean;
    /** Allow default imports from modules with no default export. This does not
     * affect code emit, just typechecking. Defaults to `false`. */
    allowSyntheticDefaultImports?: boolean;
    /** Allow accessing UMD globals from modules. Defaults to `false`. */
    allowUmdGlobalAccess?: boolean;
    /** Do not report errors on unreachable code. Defaults to `false`. */
    allowUnreachableCode?: boolean;
    /** Do not report errors on unused labels. Defaults to `false` */
    allowUnusedLabels?: boolean;
    /** Parse in strict mode and emit `"use strict"` for each source file.
     * Defaults to `true`. */
    alwaysStrict?: boolean;
    /** Base directory to resolve non-relative module names. Defaults to
     * `undefined`. */
    baseUrl?: string;
    /** The character set of the input files. Defaults to `"utf8"`. */
    charset?: string;
    /** Report errors in `.js` files. Use in conjunction with `allowJs`. Defaults
     * to `false`. */
    checkJs?: boolean;
    /** Generates corresponding `.d.ts` file. Defaults to `false`. */
    declaration?: boolean;
    /** Output directory for generated declaration files. */
    declarationDir?: string;
    /** Generates a source map for each corresponding `.d.ts` file. Defaults to
     * `false`. */
    declarationMap?: boolean;
    /** Provide full support for iterables in `for..of`, spread and
     * destructuring when targeting ES5 or ES3. Defaults to `false`. */
    downlevelIteration?: boolean;
    /** Only emit `.d.ts` declaration files. Defaults to `false`. */
    emitDeclarationOnly?: boolean;
    /** Emit design-type metadata for decorated declarations in source. See issue
     * [microsoft/TypeScript#2577](https://github.com/Microsoft/TypeScript/issues/2577)
     * for details. Defaults to `false`. */
    emitDecoratorMetadata?: boolean;
    /** Emit `__importStar` and `__importDefault` helpers for runtime babel
     * ecosystem compatibility and enable `allowSyntheticDefaultImports` for type
     * system compatibility. Defaults to `true`. */
    esModuleInterop?: boolean;
    /** Enables experimental support for ES decorators. Defaults to `true`. */
    experimentalDecorators?: boolean;
    /** Import emit helpers (e.g. `__extends`, `__rest`, etc..) from
     * [tslib](https://www.npmjs.com/package/tslib). */
    importHelpers?: boolean;
    /** This flag controls how `import` works, there are 3 different options:
     *
     * - `remove`: The default behavior of dropping import statements which only
     *   reference types.
     * - `preserve`: Preserves all `import` statements whose values or types are
     *   never used. This can cause imports/side-effects to be preserved.
     * - `error`: This preserves all imports (the same as the preserve option),
     *   but will error when a value import is only used as a type. This might
     *   be useful if you want to ensure no values are being accidentally
     *   imported, but still make side-effect imports explicit.
     *
     * This flag works because you can use `import type` to explicitly create an
     * `import` statement which should never be emitted into JavaScript. */
    importsNotUsedAsValues?: "remove" | "preserve" | "error";
    /** Emit a single file with source maps instead of having a separate file.
     * Defaults to `false`. */
    inlineSourceMap?: boolean;
    /** Emit the source alongside the source maps within a single file; requires
     * `inlineSourceMap` or `sourceMap` to be set. Defaults to `false`. */
    inlineSources?: boolean;
    /** Support JSX in `.tsx` files: `"react"`, `"preserve"`, `"react-native"`,
     * `"react-jsx", `"react-jsxdev"`.
     * Defaults to `"react"`. */
    jsx?: "react" | "preserve" | "react-native" | "react-jsx" | "react-jsx-dev";
    /** Specify the JSX factory function to use when targeting react JSX emit,
     * e.g. `React.createElement` or `h`. Defaults to `React.createElement`. */
    jsxFactory?: string;
    /** Specify the JSX fragment factory function to use when targeting react
     * JSX emit, e.g. `Fragment`. Defaults to `React.Fragment`. */
    jsxFragmentFactory?: string;
    /** Declares the module specifier to be used for importing the `jsx` and
     * `jsxs` factory functions when using jsx as `"react-jsx"` or
     * `"react-jsxdev"`. Defaults to `"react"`. */
    jsxImportSource?: string;
    /** Resolve keyof to string valued property names only (no numbers or
     * symbols). Defaults to `false`. */
    keyofStringsOnly?: string;
    /** List of library files to be included in the compilation. If omitted,
     * then the Deno main runtime libs are used. */
    lib?: string[];
    /** The locale to use to show error messages. */
    locale?: string;
    /** Specifies the location where debugger should locate map files instead of
     * generated locations. Use this flag if the `.map` files will be located at
     * run-time in a different location than the `.js` files. The location
     * specified will be embedded in the source map to direct the debugger where
     * the map files will be located. Defaults to `undefined`. */
    mapRoot?: string;
    /** Specify the module format for the emitted code. Defaults to
     * `"esnext"`. */
    module?:
      | "none"
      | "commonjs"
      | "amd"
      | "system"
      | "umd"
      | "es6"
      | "es2015"
      | "es2020"
      | "esnext";
    /** Do not generate custom helper functions like `__extends` in compiled
     * output. Defaults to `false`. */
    noEmitHelpers?: boolean;
    /** Report errors for fallthrough cases in switch statement. Defaults to
     * `false`. */
    noFallthroughCasesInSwitch?: boolean;
    /** Raise error on expressions and declarations with an implied any type.
     * Defaults to `true`. */
    noImplicitAny?: boolean;
    /** Report an error when not all code paths in function return a value.
     * Defaults to `false`. */
    noImplicitReturns?: boolean;
    /** Raise error on `this` expressions with an implied `any` type. Defaults to
     * `true`. */
    noImplicitThis?: boolean;
    /** Do not emit `"use strict"` directives in module output. Defaults to
     * `false`. */
    noImplicitUseStrict?: boolean;
    /** Do not include the default library file (`lib.d.ts`). Defaults to
     * `false`. */
    noLib?: boolean;
    /** Do not add triple-slash references or module import targets to the list of
     * compiled files. Defaults to `false`. */
    noResolve?: boolean;
    /** Disable strict checking of generic signatures in function types. Defaults
     * to `false`. */
    noStrictGenericChecks?: boolean;
    /** Include 'undefined' in index signature results. Defaults to `false`. */
    noUncheckedIndexedAccess?: boolean;
    /** Report errors on unused locals. Defaults to `false`. */
    noUnusedLocals?: boolean;
    /** Report errors on unused parameters. Defaults to `false`. */
    noUnusedParameters?: boolean;
    /** List of path mapping entries for module names to locations relative to the
     * `baseUrl`. Defaults to `undefined`. */
    paths?: Record<string, string[]>;
    /** Do not erase const enum declarations in generated code. Defaults to
     * `false`. */
    preserveConstEnums?: boolean;
    /** Remove all comments except copy-right header comments beginning with
     * `/*!`. Defaults to `true`. */
    removeComments?: boolean;
    /** Specifies the root directory of input files. Only use to control the
     * output directory structure with `outDir`. Defaults to `undefined`. */
    rootDir?: string;
    /** List of _root_ folders whose combined content represent the structure of
     * the project at runtime. Defaults to `undefined`. */
    rootDirs?: string[];
    /** Generates corresponding `.map` file. Defaults to `false`. */
    sourceMap?: boolean;
    /** Specifies the location where debugger should locate TypeScript files
     * instead of source locations. Use this flag if the sources will be located
     * at run-time in a different location than that at design-time. The location
     * specified will be embedded in the sourceMap to direct the debugger where
     * the source files will be located. Defaults to `undefined`. */
    sourceRoot?: string;
    /** Skip type checking of all declaration files (`*.d.ts`). */
    skipLibCheck?: boolean;
    /** Enable all strict type checking options. Enabling `strict` enables
     * `noImplicitAny`, `noImplicitThis`, `alwaysStrict`, `strictBindCallApply`,
     * `strictNullChecks`, `strictFunctionTypes` and
     * `strictPropertyInitialization`. Defaults to `true`. */
    strict?: boolean;
    /** Enable stricter checking of the `bind`, `call`, and `apply` methods on
     * functions. Defaults to `true`. */
    strictBindCallApply?: boolean;
    /** Disable bivariant parameter checking for function types. Defaults to
     * `true`. */
    strictFunctionTypes?: boolean;
    /** Ensure non-undefined class properties are initialized in the constructor.
     * This option requires `strictNullChecks` be enabled in order to take effect.
     * Defaults to `true`. */
    strictPropertyInitialization?: boolean;
    /** In strict null checking mode, the `null` and `undefined` values are not in
     * the domain of every type and are only assignable to themselves and `any`
     * (the one exception being that `undefined` is also assignable to `void`). */
    strictNullChecks?: boolean;
    /** Suppress excess property checks for object literals. Defaults to
     * `false`. */
    suppressExcessPropertyErrors?: boolean;
    /** Suppress `noImplicitAny` errors for indexing objects lacking index
     * signatures. */
    suppressImplicitAnyIndexErrors?: boolean;
    /** Specify ECMAScript target version. Defaults to `esnext`. */
    target?:
      | "es3"
      | "es5"
      | "es6"
      | "es2015"
      | "es2016"
      | "es2017"
      | "es2018"
      | "es2019"
      | "es2020"
      | "esnext";
    /** List of names of type definitions to include when type checking.
     * Defaults to `undefined`.
     *
     * The type definitions are resolved according to the normal Deno resolution
     * irrespective of if sources are provided on the call. In addition, unlike
     * passing the `--config` option on startup, there is no base to resolve
     * relative specifiers, so the specifiers here have to be fully qualified
     * URLs or paths.  For example:
     *
     * ```ts
     * Deno.emit("./a.ts", {
     *   compilerOptions: {
     *     types: [
     *       "https://deno.land/x/pkg/types.d.ts",
     *       "/Users/me/pkg/types.d.ts",
     *     ]
     *   }
     * });
     * ```
     */
    types?: string[];
    /** Emit class fields with ECMAScript-standard semantics. Defaults to
     * `false`. */
    useDefineForClassFields?: boolean;
  }

  interface ImportMap {
    imports: Record<string, string>;
    scopes?: Record<string, Record<string, string>>;
  }

  /**
   * **UNSTABLE**: new API, yet to be vetted.
   *
   * The options for `Deno.emit()` API.
   */
  export interface EmitOptions {
    /** Indicate that the source code should be emitted to a single file
     * JavaScript bundle that is a single ES module (`"module"`) or a single
     * file self contained script executed in an immediately invoked function
     * when loaded (`"classic"`). */
    bundle?: "module" | "classic";
    /** If `true` then the sources will be typed checked, returning any
     * diagnostic errors in the result.  If `false` type checking will be
     * skipped.  Defaults to `true`.
     *
     * *Note* by default, only TypeScript will be type checked, just like on
     * the command line.  Use the `compilerOptions` options of `checkJs` to
     * enable type checking of JavaScript. */
    check?: boolean;
    /** A set of options that are aligned to TypeScript compiler options that
     * are supported by Deno. */
    compilerOptions?: CompilerOptions;
    /** An [import-map](https://deno.land/manual/linking_to_external_code/import_maps#import-maps)
     * which will be applied to the imports. */
    importMap?: ImportMap;
    /** An absolute path to an [import-map](https://deno.land/manual/linking_to_external_code/import_maps#import-maps).
     * Required to be specified if an `importMap` is specified to be able to
     * determine resolution of relative paths. If a `importMap` is not
     * specified, then it will assumed the file path points to an import map on
     * disk and will be attempted to be loaded based on current runtime
     * permissions.
     */
    importMapPath?: string;
    /** A record of sources to use when doing the emit.  If provided, Deno will
     * use these sources instead of trying to resolve the modules externally. */
    sources?: Record<string, string>;
  }

  /**
   * **UNSTABLE**: new API, yet to be vetted.
   *
   * The result of `Deno.emit()` API.
   */
  export interface EmitResult {
    /** Diagnostic messages returned from the type checker (`tsc`).
     *
     * Can be used with `Deno.formatDiagnostics` to display a user
     * friendly string. */
    diagnostics: Diagnostic[];
    /** Any emitted files.  If bundled, then the JavaScript will have the
     * key of `deno:///bundle.js` with an optional map (based on
     * `compilerOptions`) in `deno:///bundle.js.map`. */
    files: Record<string, string>;
    /** An optional array of any compiler options that were ignored by Deno. */
    ignoredOptions?: string[];
    /** An array of internal statistics related to the emit, for diagnostic
     * purposes. */
    stats: Array<[string, number]>;
  }

  /**
   * **UNSTABLE**: new API, yet to be vetted.
   *
   * Similar to the command line functionality of `deno run` or `deno cache`,
   * `Deno.emit()` provides a way to provide Deno arbitrary JavaScript
   * or TypeScript and have it return JavaScript based on the options and
   * settings provided. The source code can either be provided or the modules
   * can be fetched and resolved in line with the behavior of the command line.
   *
   * Requires `allow-read` and/or `allow-net` if sources are not provided.
   *
   * @param rootSpecifier The specifier that will be used as the entry point.
   *                      If no sources are provided, then the specifier would
   *                      be the same as if you typed it on the command line for
   *                      `deno run`. If sources are provided, it should match
   *                      one of the names of the sources.
   * @param options  A set of options to be used with the emit.
   *
   * @returns The result of the emit. If diagnostics are found, they can be used
   * with `Deno.formatDiagnostics` to construct a user friendly string, which
   * has the same format as CLI diagnostics.
   */
  export function emit(
    rootSpecifier: string | URL,
    options?: EmitOptions,
  ): Promise<EmitResult>;

  /** **UNSTABLE**: Should not have same name as `window.location` type. */
  interface Location {
    /** The full url for the module, e.g. `file://some/file.ts` or
     * `https://some/file.ts`. */
    fileName: string;
    /** The line number in the file. It is assumed to be 1-indexed. */
    lineNumber: number;
    /** The column number in the file. It is assumed to be 1-indexed. */
    columnNumber: number;
  }

  /** **UNSTABLE**: new API, yet to be vetted.
   *
   * Given a current location in a module, lookup the source location and return
   * it.
   *
   * When Deno transpiles code, it keep source maps of the transpiled code. This
   * function can be used to lookup the original location. This is
   * automatically done when accessing the `.stack` of an error, or when an
   * uncaught error is logged. This function can be used to perform the lookup
   * for creating better error handling.
   *
   * **Note:** `lineNumber` and `columnNumber` are 1 indexed, which matches display
   * expectations, but is not typical of most index numbers in Deno.
   *
   * An example:
   *
   * ```ts
   * const origin = Deno.applySourceMap({
   *   fileName: "file://my/module.ts",
   *   lineNumber: 5,
   *   columnNumber: 15
   * });
   *
   * console.log(`${origin.fileName}:${origin.lineNumber}:${origin.columnNumber}`);
   * ```
   */
  export function applySourceMap(location: Location): Location;

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
   * Requires `allow-write` permission. */
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
   * Requires `allow-write` permission. */
  export function utime(
    path: string | URL,
    atime: number | Date,
    mtime: number | Date,
  ): Promise<void>;

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
   */
  export class HttpClient {
    rid: number;
    close(): void;
  }

  /** **UNSTABLE**: New API, yet to be vetted.
   * The options used when creating a [HttpClient].
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

  export interface Proxy {
    url: string;
    basicAuth?: BasicAuth;
  }

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
   */
  export function futime(
    rid: number,
    atime: number | Date,
    mtime: number | Date,
  ): Promise<void>;

  /** **UNSTABLE**: new API, yet to be vetted.
   *
   * SleepSync puts the main thread to sleep synchronously for a given amount of
   * time in milliseconds.
   *
   * ```ts
   * Deno.sleepSync(10);
   * ```
   */
  export function sleepSync(millis: number): void;

  /** **UNSTABLE**: new API, yet to be vetted.
   *
   * A generic transport listener for message-oriented protocols. */
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
   * Requires `allow-read` and `allow-write` permission. */
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
   * Requires `allow-net` permission. */
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
   * Requires `allow-read` and `allow-write` permission. */
  export function listenDatagram(
    options: UnixListenOptions & { transport: "unixpacket" },
  ): DatagramConn;

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
   * Requires `allow-net` permission for "tcp" and `allow-read` for "unix". */
  export function connect(
    options: ConnectOptions,
  ): Promise<TcpConn>;
  export function connect(
    options: UnixConnectOptions,
  ): Promise<Conn>;

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

  export interface TlsHandshakeInfo {
    /** **UNSTABLE**: new API, yet to be vetted.
     *
     * Contains the ALPN protocol selected during negotiation with the server.
     * If no ALPN protocol selected, returns `null`.
     */
    alpnProtocol: string | null;
  }

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
   */
  export function connectTls(options: ConnectTlsOptions): Promise<TlsConn>;

  export interface ListenTlsOptions {
    /** **UNSTABLE**: new API, yet to be vetted.
     *
     * Application-Layer Protocol Negotiation (ALPN) protocols to announce to
     * the client. If not specified, no ALPN extension will be included in the
     * TLS handshake.
     */
    alpnProtocols?: string[];
  }

  export interface StartTlsOptions {
    /** **UNSTABLE**: new API, yet to be vetted.
     *
     * Application-Layer Protocol Negotiation (ALPN) protocols to announce to
     * the client. If not specified, no ALPN extension will be included in the
     * TLS handshake.
     */
    alpnProtocols?: string[];
  }

  /** **UNSTABLE**: New API should be tested first.
   *
   * Acquire an advisory file-system lock for the provided file. `exclusive`
   * defaults to `false`.
   */
  export function flock(rid: number, exclusive?: boolean): Promise<void>;

  /** **UNSTABLE**: New API should be tested first.
   *
   * Acquire an advisory file-system lock for the provided file. `exclusive`
   * defaults to `false`.
   */
  export function flockSync(rid: number, exclusive?: boolean): void;

  /** **UNSTABLE**: New API should be tested first.
   *
   * Release an advisory file-system lock for the provided file.
   */
  export function funlock(rid: number): Promise<void>;

  /** **UNSTABLE**: New API should be tested first.
   *
   * Release an advisory file-system lock for the provided file.
   */
  export function funlockSync(rid: number): void;

  /** **UNSTABLE**: new API, yet to be vetted.
   *
   * Make the timer of the given id blocking the event loop from finishing
   */
  export function refTimer(id: number): void;

  /** **UNSTABLE**: new API, yet to be vetted.
   *
   * Make the timer of the given id not blocking the event loop from finishing
   */
  export function unrefTimer(id: number): void;
}

declare function fetch(
  input: Request | URL | string,
  init?: RequestInit & { client: Deno.HttpClient },
): Promise<Response>;

declare interface WorkerOptions {
  /** UNSTABLE: New API.
   *
   * Set deno.namespace to `true` to make `Deno` namespace and all of its
   * methods available to the worker environment. Defaults to `false`.
   *
   * Configure deno.permissions options to change the level of access the worker will
   * have. By default it will inherit the permissions of its parent thread. The permissions
   * of a worker can't be extended beyond its parent's permissions reach.
   * - "inherit" will take the permissions of the thread the worker is created in
   * - You can disable/enable permissions all together by passing a boolean
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
   *       namespace: true,
   *       permissions: {
   *         read: true,
   *       },
   *     },
   *   }
   * );
   * ```
   */
  // TODO(Soremwar)
  // `deno: boolean` is kept for backwards compatibility with the previous
  // worker options implementation. Remove for 2.0.
  deno?: boolean | {
    namespace?: boolean;
    /** Set to `"none"` to disable all the permissions in the worker. */
    permissions?: "inherit" | "none" | {
      env?: "inherit" | boolean | string[];
      hrtime?: "inherit" | boolean;
      /** The format of the net access list must be `hostname[:port]`
       * in order to be resolved.
       *
       * For example: `["https://deno.land", "localhost:8080"]`.
       */
      net?: "inherit" | boolean | string[];
      ffi?: "inherit" | boolean | Array<string | URL>;
      read?: "inherit" | boolean | Array<string | URL>;
      run?: "inherit" | boolean | Array<string | URL>;
      write?: "inherit" | boolean | Array<string | URL>;
    };
  };
}

declare interface WebSocketStreamOptions {
  protocols?: string[];
  signal?: AbortSignal;
  headers?: HeadersInit;
}

declare interface WebSocketConnection {
  readable: ReadableStream<string | Uint8Array>;
  writable: WritableStream<string | Uint8Array>;
  extensions: string;
  protocol: string;
}

declare interface WebSocketCloseInfo {
  code?: number;
  reason?: string;
}

declare class WebSocketStream {
  constructor(url: string, options?: WebSocketStreamOptions);
  url: string;
  connection: Promise<WebSocketConnection>;
  closed: Promise<WebSocketCloseInfo>;
  close(closeInfo?: WebSocketCloseInfo): void;
}
