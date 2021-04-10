// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

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
   *
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

  /** **Unstable** new API. yet to be vetted.
   *
   * Returns the total number of logical cpus in the system along with
   * the speed measured in MHz. If either the syscall to get the core
   * count or speed of the cpu is unsuccessful the value of the it
   * is undefined.
   *
   * ```ts
   * console.log(Deno.systemCpuInfo());
   * ```
   *
   * Requires `allow-env` permission.
   *
   */
  export function systemCpuInfo(): SystemCpuInfo;

  export interface SystemCpuInfo {
    /** Total number of logical cpus in the system */
    cores: number | undefined;
    /** The speed of the cpu measured in MHz */
    speed: number | undefined;
  }

  /** **UNSTABLE**: new API, yet to be vetted.
   *
   * Open and initialize a plugin.
   *
   * ```ts
   * const rid = Deno.openPlugin("./path/to/some/plugin.so");
   * const opId = Deno.core.ops()["some_op"];
   * const response = Deno.core.dispatch(opId, new Uint8Array([1,2,3,4]));
   * console.log(`Response from plugin ${response}`);
   * ```
   *
   * Requires `allow-plugin` permission.
   *
   * The plugin system is not stable and will change in the future, hence the
   * lack of docs. For now take a look at the example
   * https://github.com/denoland/deno/tree/master/test_plugin
   */
  export function openPlugin(filename: string): number;

  /** The log category for a diagnostic message. */
  export enum DiagnosticCategory {
    Warning = 0,
    Error = 1,
    Suggestion = 2,
    Message = 3,
  }

  export interface DiagnosticMessageChain {
    message: string;
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
   * user friendly format.
   *
   * ```ts
   * const { diagnostics } = await Deno.emit("file_with_compile_issues.ts");
   * console.table(diagnostics);  // Prints raw diagnostic data
   * console.log(Deno.formatDiagnostics(diagnostics));  // User friendly output of diagnostics
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
    /** Support JSX in `.tsx` files: `"react"`, `"preserve"`, `"react-native"`.
     * Defaults to `"react"`. */
    jsx?: "react" | "preserve" | "react-native";
    /** Specify the JSX factory function to use when targeting react JSX emit,
     * e.g. `React.createElement` or `h`. Defaults to `React.createElement`. */
    jsxFactory?: string;
    /** Specify the JSX fragment factory function to use when targeting react
     * JSX emit, e.g. `Fragment`. Defaults to `React.Fragment`. */
    jsxFragmentFactory?: string;
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
    /** List of names of type definitions to include. Defaults to `undefined`.
     *
     * The type definitions are resolved according to the normal Deno resolution
     * irrespective of if sources are provided on the call. Like other Deno
     * modules, there is no "magical" resolution. For example:
     *
     * ```ts
     * Deno.compile(
     *   "./foo.js",
     *   undefined,
     *   {
     *     types: [ "./foo.d.ts", "https://deno.land/x/example/types.d.ts" ]
     *   }
     * );
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
     * JavaScript bundle that is a single ES module (`"esm"`) or a single file
     * self contained script we executes in an immediately invoked function
     * when loaded (`"iife"`). */
    bundle?: "esm" | "iife";
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
    /** Diagnostic messages returned from the type checker (`tsc`). */
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
   * **Note:** `line` and `column` are 1 indexed, which matches display
   * expectations, but is not typical of most index numbers in Deno.
   *
   * An example:
   *
   * ```ts
   * const orig = Deno.applySourceMap({
   *   fileName: "file://my/module.ts",
   *   lineNumber: 5,
   *   columnNumber: 15
   * });
   * console.log(`${orig.filename}:${orig.line}:${orig.column}`);
   * ```
   */
  export function applySourceMap(location: Location): Location;

  enum LinuxSignal {
    SIGHUP = 1,
    SIGINT = 2,
    SIGQUIT = 3,
    SIGILL = 4,
    SIGTRAP = 5,
    SIGABRT = 6,
    SIGBUS = 7,
    SIGFPE = 8,
    SIGKILL = 9,
    SIGUSR1 = 10,
    SIGSEGV = 11,
    SIGUSR2 = 12,
    SIGPIPE = 13,
    SIGALRM = 14,
    SIGTERM = 15,
    SIGSTKFLT = 16,
    SIGCHLD = 17,
    SIGCONT = 18,
    SIGSTOP = 19,
    SIGTSTP = 20,
    SIGTTIN = 21,
    SIGTTOU = 22,
    SIGURG = 23,
    SIGXCPU = 24,
    SIGXFSZ = 25,
    SIGVTALRM = 26,
    SIGPROF = 27,
    SIGWINCH = 28,
    SIGIO = 29,
    SIGPWR = 30,
    SIGSYS = 31,
  }
  enum MacOSSignal {
    SIGHUP = 1,
    SIGINT = 2,
    SIGQUIT = 3,
    SIGILL = 4,
    SIGTRAP = 5,
    SIGABRT = 6,
    SIGEMT = 7,
    SIGFPE = 8,
    SIGKILL = 9,
    SIGBUS = 10,
    SIGSEGV = 11,
    SIGSYS = 12,
    SIGPIPE = 13,
    SIGALRM = 14,
    SIGTERM = 15,
    SIGURG = 16,
    SIGSTOP = 17,
    SIGTSTP = 18,
    SIGCONT = 19,
    SIGCHLD = 20,
    SIGTTIN = 21,
    SIGTTOU = 22,
    SIGIO = 23,
    SIGXCPU = 24,
    SIGXFSZ = 25,
    SIGVTALRM = 26,
    SIGPROF = 27,
    SIGWINCH = 28,
    SIGINFO = 29,
    SIGUSR1 = 30,
    SIGUSR2 = 31,
  }

  /** **UNSTABLE**: Further changes required to make platform independent.
   *
   * Signals numbers. This is platform dependent. */
  export const Signal: typeof MacOSSignal | typeof LinuxSignal;

  /** **UNSTABLE**: new API, yet to be vetted.
   *
   * Represents the stream of signals, implements both `AsyncIterator` and
   * `PromiseLike`. */
  export class SignalStream
    implements AsyncIterableIterator<void>, PromiseLike<void> {
    constructor(signal: typeof Deno.Signal);
    then<T, S>(
      f: (v: void) => T | Promise<T>,
      g?: (v: void) => S | Promise<S>,
    ): Promise<T | S>;
    next(): Promise<IteratorResult<void>>;
    [Symbol.asyncIterator](): AsyncIterableIterator<void>;
    dispose(): void;
  }

  /** **UNSTABLE**: new API, yet to be vetted.
   *
   * Returns the stream of the given signal number. You can use it as an async
   * iterator.
   *
   * ```ts
   * for await (const _ of Deno.signal(Deno.Signal.SIGTERM)) {
   *   console.log("got SIGTERM!");
   * }
   * ```
   *
   * You can also use it as a promise. In this case you can only receive the
   * first one.
   *
   * ```ts
   * await Deno.signal(Deno.Signal.SIGTERM);
   * console.log("SIGTERM received!")
   * ```
   *
   * If you want to stop receiving the signals, you can use `.dispose()` method
   * of the signal stream object.
   *
   * ```ts
   * const sig = Deno.signal(Deno.Signal.SIGTERM);
   * setTimeout(() => { sig.dispose(); }, 5000);
   * for await (const _ of sig) {
   *   console.log("SIGTERM!")
   * }
   * ```
   *
   * The above for-await loop exits after 5 seconds when `sig.dispose()` is
   * called.
   *
   * NOTE: This functionality is not yet implemented on Windows.
   */
  export function signal(signo: number): SignalStream;

  /** **UNSTABLE**: new API, yet to be vetted. */
  export const signals: {
    /** Returns the stream of SIGALRM signals.
     *
     * This method is the shorthand for `Deno.signal(Deno.Signal.SIGALRM)`. */
    alarm: () => SignalStream;
    /** Returns the stream of SIGCHLD signals.
     *
     * This method is the shorthand for `Deno.signal(Deno.Signal.SIGCHLD)`. */
    child: () => SignalStream;
    /** Returns the stream of SIGHUP signals.
     *
     * This method is the shorthand for `Deno.signal(Deno.Signal.SIGHUP)`. */
    hungup: () => SignalStream;
    /** Returns the stream of SIGINT signals.
     *
     * This method is the shorthand for `Deno.signal(Deno.Signal.SIGINT)`. */
    interrupt: () => SignalStream;
    /** Returns the stream of SIGIO signals.
     *
     * This method is the shorthand for `Deno.signal(Deno.Signal.SIGIO)`. */
    io: () => SignalStream;
    /** Returns the stream of SIGPIPE signals.
     *
     * This method is the shorthand for `Deno.signal(Deno.Signal.SIGPIPE)`. */
    pipe: () => SignalStream;
    /** Returns the stream of SIGQUIT signals.
     *
     * This method is the shorthand for `Deno.signal(Deno.Signal.SIGQUIT)`. */
    quit: () => SignalStream;
    /** Returns the stream of SIGTERM signals.
     *
     * This method is the shorthand for `Deno.signal(Deno.Signal.SIGTERM)`. */
    terminate: () => SignalStream;
    /** Returns the stream of SIGUSR1 signals.
     *
     * This method is the shorthand for `Deno.signal(Deno.Signal.SIGUSR1)`. */
    userDefined1: () => SignalStream;
    /** Returns the stream of SIGUSR2 signals.
     *
     * This method is the shorthand for `Deno.signal(Deno.Signal.SIGUSR2)`. */
    userDefined2: () => SignalStream;
    /** Returns the stream of SIGWINCH signals.
     *
     * This method is the shorthand for `Deno.signal(Deno.Signal.SIGWINCH)`. */
    windowChange: () => SignalStream;
  };

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
   * Deno.setRaw(myTTY.rid, true, { cbreak: true });
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
    path: string,
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
    path: string,
    atime: number | Date,
    mtime: number | Date,
  ): Promise<void>;

  /** The type of the resource record.
   * Only the listed types are supported currently. */
  export type RecordType =
    | "A"
    | "AAAA"
    | "ANAME"
    | "CNAME"
    | "MX"
    | "PTR"
    | "SRV"
    | "TXT";

  export interface ResolveDnsOptions {
    /** The name server to be used for lookups.
     * If not specified, defaults to the system configuration e.g. `/etc/resolv.conf` on Unix. */
    nameServer?: {
      /** The IP address of the name server */
      ipAddr: string;
      /** The port number the query will be sent to.
       * If not specified, defaults to 53. */
      port?: number;
    };
  }

  /** If `resolveDns` is called with "MX" record type specified, it will return an array of this interface. */
  export interface MXRecord {
    preference: number;
    exchange: string;
  }

  /** If `resolveDns` is called with "SRV" record type specified, it will return an array of this interface. */
  export interface SRVRecord {
    priority: number;
    weight: number;
    port: number;
    target: string;
  }

  export function resolveDns(
    query: string,
    recordType: "A" | "AAAA" | "ANAME" | "CNAME" | "PTR",
    options?: ResolveDnsOptions,
  ): Promise<string[]>;

  export function resolveDns(
    query: string,
    recordType: "MX",
    options?: ResolveDnsOptions,
  ): Promise<MXRecord[]>;

  export function resolveDns(
    query: string,
    recordType: "SRV",
    options?: ResolveDnsOptions,
  ): Promise<SRVRecord[]>;

  export function resolveDns(
    query: string,
    recordType: "TXT",
    options?: ResolveDnsOptions,
  ): Promise<string[][]>;

  /** ** UNSTABLE**: new API, yet to be vetted.
   *
   * Performs DNS resolution against the given query, returning resolved records.
   * Fails in the cases such as:
   * - the query is in invalid format
   * - the options have an invalid parameter, e.g. `nameServer.port` is beyond the range of 16-bit unsigned integer
   * - timed out
   *
   * ```ts
   * const a = await Deno.resolveDns("example.com", "A");
   *
   * const aaaa = await Deno.resolveDns("example.com", "AAAA", {
   *   nameServer: { ipAddr: "8.8.8.8", port: 1234 },
   * });
   * ```
   *
   * Requires `allow-net` permission.
   */
  export function resolveDns(
    query: string,
    recordType: RecordType,
    options?: ResolveDnsOptions,
  ): Promise<string[] | MXRecord[] | SRVRecord[] | string[][]>;

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
   *   address: "/foo/bar.sock",
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
    options: ConnectOptions | UnixConnectOptions,
  ): Promise<Conn>;

  export interface StartTlsOptions {
    /** A literal IP address or host name that can be resolved to an IP address.
     * If not specified, defaults to `127.0.0.1`. */
    hostname?: string;
    /** Server certificate file. */
    certFile?: string;
  }

  /** **UNSTABLE**: new API, yet to be vetted.
   *
   * Start TLS handshake from an existing connection using
   * an optional cert file, hostname (default is "127.0.0.1").  The
   * cert file is optional and if not included Mozilla's root certificates will
   * be used (see also https://github.com/ctz/webpki-roots for specifics)
   * Using this function requires that the other end of the connection is
   * prepared for TLS handshake.
   *
   * ```ts
   * const conn = await Deno.connect({ port: 80, hostname: "127.0.0.1" });
   * const tlsConn = await Deno.startTls(conn, { certFile: "./certs/my_custom_root_CA.pem", hostname: "127.0.0.1", port: 80 });
   * ```
   *
   * Requires `allow-net` permission.
   */
  export function startTls(
    conn: Conn,
    options?: StartTlsOptions,
  ): Promise<Conn>;

  /** **UNSTABLE**: The `signo` argument may change to require the Deno.Signal
   * enum.
   *
   * Send a signal to process under given `pid`. This functionality currently
   * only works on Linux and Mac OS.
   *
   * If `pid` is negative, the signal will be sent to the process group
   * identified by `pid`.
   *
   *      const p = Deno.run({
   *        cmd: ["sleep", "10000"]
   *      });
   *
   *      Deno.kill(p.pid, Deno.Signal.SIGINT);
   *
   * Requires `allow-run` permission. */
  export function kill(pid: number, signo: number): void;

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

  /** **UNSTABLE**: new API, yet to be vetted.
   * Synchronously truncates or extends the specified file stream, to reach the
   * specified `len`.  If `len` is not specified then the entire file contents
   * are truncated.
   *
   * ```ts
   * // truncate the entire file
   * const file = Deno.open("my_file.txt", { read: true, write: true, truncate: true, create: true });
   * Deno.ftruncateSync(file.rid);
   *
   * // truncate part of the file
   * const file = Deno.open("my_file.txt", { read: true, write: true, create: true });
   * Deno.write(file.rid, new TextEncoder().encode("Hello World"));
   * Deno.ftruncateSync(file.rid, 7);
   * const data = new Uint8Array(32);
   * Deno.readSync(file.rid, data);
   * console.log(new TextDecoder().decode(data)); // Hello W
   * ```
   */
  export function ftruncateSync(rid: number, len?: number): void;

  /** **UNSTABLE**: new API, yet to be vetted.
   * Truncates or extends the specified file stream, to reach the specified `len`. If
   * `len` is not specified then the entire file contents are truncated.
   *
   * ```ts
   * // truncate the entire file
   * const file = Deno.open("my_file.txt", { read: true, write: true, create: true });
   * await Deno.ftruncate(file.rid);
   *
   * // truncate part of the file
   * const file = Deno.open("my_file.txt", { read: true, write: true, create: true });
   * await Deno.write(file.rid, new TextEncoder().encode("Hello World"));
   * await Deno.ftruncate(file.rid, 7);
   * const data = new Uint8Array(32);
   * await Deno.read(file.rid, data);
   * console.log(new TextDecoder().decode(data)); // Hello W
   * ```
   */
  export function ftruncate(rid: number, len?: number): Promise<void>;

  /** **UNSTABLE**: New API, yet to be vetted.
   * Synchronously returns a `Deno.FileInfo` for the given file stream.
   *
   * ```ts
   * const file = Deno.openSync("file.txt", { read: true });
   * const fileInfo = Deno.fstatSync(file.rid);
   * assert(fileInfo.isFile);
   * ```
   */
  export function fstatSync(rid: number): FileInfo;

  /** **UNSTABLE**: New API, yet to be vetted.
   * Returns a `Deno.FileInfo` for the given file stream.
   *
   * ```ts
   * const file = await Deno.open("file.txt", { read: true });
   * const fileInfo = await Deno.fstat(file.rid);
   * assert(fileInfo.isFile);
   * ```
   */
  export function fstat(rid: number): Promise<FileInfo>;

  /** **UNSTABLE**: New API, yet to be vetted.
   * The pid of the current process's parent.
   */
  export const ppid: number;

  /** **UNSTABLE**: New API, yet to be vetted.
   * A custom HttpClient for use with `fetch`.
   *
   * ```ts
   * const client = new Deno.createHttpClient({ caFile: "./ca.pem" });
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
    /** A certificate authority to use when validating TLS certificates. Certificate data must be PEM encoded.
     */
    caData?: string;
  }

  /** **UNSTABLE**: New API, yet to be vetted.
   * Create a custom HttpClient for to use with `fetch`.
   *
   * ```ts
   * const client = new Deno.createHttpClient({ caFile: "./ca.pem" });
   * const req = await fetch("https://myserver.com", { client });
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

  /** *UNSTABLE**: new API, yet to be vetted.
   *
   * SleepSync puts the main thread to sleep synchronously for a given amount of
   * time in milliseconds.
   *
   * ```ts
   * Deno.sleepSync(10);
   * ```
   */
  export function sleepSync(millis: number): Promise<void>;

  export interface Metrics extends OpMetrics {
    ops: Record<string, OpMetrics>;
  }

  export interface OpMetrics {
    opsDispatched: number;
    opsDispatchedSync: number;
    opsDispatchedAsync: number;
    opsDispatchedAsyncUnref: number;
    opsCompleted: number;
    opsCompletedSync: number;
    opsCompletedAsync: number;
    opsCompletedAsyncUnref: number;
    bytesSentControl: number;
    bytesSentData: number;
    bytesReceived: number;
  }

  export interface MemoryUsage {
    rss: number;
    heapTotal: number;
    heapUsed: number;
    external: number;
  }

  export function memoryUsage(): MemoryUsage;

  export interface RequestEvent {
    readonly request: Request;
    respondWith(r: Response | Promise<Response>): void;
  }

  export interface HttpConn extends AsyncIterable<RequestEvent> {
    readonly rid: number;

    nextRequest(): Promise<RequestEvent | null>;
    close(): void;
  }

  /** **UNSTABLE**: new API, yet to be vetted.
   *
   * Services HTTP requests given a TCP or TLS socket.
   *
   * ```ts
   * const httpConn = Deno.serveHttp(conn);
   * const e = await httpConn.nextRequest();
   * if (e) {
   *   e.respondWith(new Response("Hello World"));
   * }
   * ```
   *
   * If `httpConn.nextRequest()` encounters an error or returns `null`
   * then the underlying HttpConn resource is closed automatically.
   */
  export function serveHttp(conn: Conn): HttpConn;
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
   * worker.postMessage({ cmd: "readFile", fileName: "./log.txt" });
   *
   * // deno_worker.ts
   *
   *
   * self.onmessage = async function (e) {
   *     const { cmd, fileName } = e.data;
   *     if (cmd !== "readFile") {
   *         throw new Error("Invalid command");
   *     }
   *     const buf = await Deno.readFile(fileName);
   *     const fileContents = new TextDecoder().decode(buf);
   *     console.log(fileContents);
   * }
   *
   * // $ cat log.txt
   * // hello world
   * // hello world 2
   *
   * // $ deno run --allow-read mod.ts
   * // hello world
   * // hello world2
   * ```
   */
  // TODO(Soremwar)
  // `deno: boolean` is kept for backwards compatibility with the previous
  // worker options implementation. Remove for 2.0.
  deno?: boolean | {
    namespace?: boolean;
    /** Set to `"none"` to disable all the permissions in the worker. */
    permissions?: "inherit" | "none" | {
      env?: "inherit" | boolean;
      hrtime?: "inherit" | boolean;
      /** The format of the net access list must be `hostname[:port]`
       * in order to be resolved.
       *
       * ```
       * net: ["https://deno.land", "localhost:8080"],
       * ```
       * */
      net?: "inherit" | boolean | string[];
      plugin?: "inherit" | boolean;
      read?: "inherit" | boolean | Array<string | URL>;
      run?: "inherit" | boolean;
      write?: "inherit" | boolean | Array<string | URL>;
    };
  };
}
