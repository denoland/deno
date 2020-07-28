// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

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

  /** **UNSTABLE**: This API needs a security review.
   *
   * Synchronously creates `newpath` as a hard link to `oldpath`.
   *
   * ```ts
   * Deno.linkSync("old/name", "new/name");
   * ```
   *
   * Requires `allow-read` and `allow-write` permissions. */
  export function linkSync(oldpath: string, newpath: string): void;

  /** **UNSTABLE**: This API needs a security review.
   *
   * Creates `newpath` as a hard link to `oldpath`.
   *
   * ```ts
   * await Deno.link("old/name", "new/name");
   * ```
   *
   * Requires `allow-read` and `allow-write` permissions. */
  export function link(oldpath: string, newpath: string): Promise<void>;

  /** **UNSTABLE**: New API, yet to be vetted.
   *
   * Gets the size of the console as columns/rows.
   *
   * ```ts
   * const { columns, rows } = await Deno.consoleSize(Deno.stdout.rid);
   * ```
   */
  export function consoleSize(
    rid: number,
  ): {
    columns: number;
    rows: number;
  };

  export type SymlinkOptions = {
    type: "file" | "dir";
  };

  /** **UNSTABLE**: This API needs a security review.
   *
   * Creates `newpath` as a symbolic link to `oldpath`.
   *
   * The options.type parameter can be set to `file` or `dir`. This argument is only
   * available on Windows and ignored on other platforms.
   *
   * ```ts
   * Deno.symlinkSync("old/name", "new/name");
   * ```
   *
   * Requires `allow-write` permission. */
  export function symlinkSync(
    oldpath: string,
    newpath: string,
    options?: SymlinkOptions,
  ): void;

  /** **UNSTABLE**: This API needs a security review.
   *
   * Creates `newpath` as a symbolic link to `oldpath`.
   *
   * The options.type parameter can be set to `file` or `dir`. This argument is only
   * available on Windows and ignored on other platforms.
   *
   * ```ts
   * await Deno.symlink("old/name", "new/name");
   * ```
   *
   * Requires `allow-write` permission. */
  export function symlink(
    oldpath: string,
    newpath: string,
    options?: SymlinkOptions,
  ): Promise<void>;

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
    Log = 0,
    Debug = 1,
    Info = 2,
    Error = 3,
    Warning = 4,
    Suggestion = 5,
  }

  export interface DiagnosticMessageChain {
    message: string;
    category: DiagnosticCategory;
    code: number;
    next?: DiagnosticMessageChain[];
  }

  export interface DiagnosticItem {
    /** A string message summarizing the diagnostic. */
    message: string;
    /** An ordered array of further diagnostics. */
    messageChain?: DiagnosticMessageChain;
    /** Information related to the diagnostic. This is present when there is a
     * suggestion or other additional diagnostic information */
    relatedInformation?: DiagnosticItem[];
    /** The text of the source line related to the diagnostic. */
    sourceLine?: string;
    /** The line number that is related to the diagnostic. */
    lineNumber?: number;
    /** The name of the script resource related to the diagnostic. */
    scriptResourceName?: string;
    /** The start position related to the diagnostic. */
    startPosition?: number;
    /** The end position related to the diagnostic. */
    endPosition?: number;
    /** The category of the diagnostic. */
    category: DiagnosticCategory;
    /** A number identifier. */
    code: number;
    /** The the start column of the sourceLine related to the diagnostic. */
    startColumn?: number;
    /** The end column of the sourceLine related to the diagnostic. */
    endColumn?: number;
  }

  export interface Diagnostic {
    /** An array of diagnostic items. */
    items: DiagnosticItem[];
  }

  /** **UNSTABLE**: new API, yet to be vetted.
   *
   * Format an array of diagnostic items and return them as a single string in a
   * user friendly format.
   *
   * ```ts
   * const [diagnostics, result] = Deno.compile("file_with_compile_issues.ts");
   * console.table(diagnostics);  // Prints raw diagnostic data
   * console.log(Deno.formatDiagnostics(diagnostics));  // User friendly output of diagnostics
   * ```
   *
   * @param items An array of diagnostic items to format
   */
  export function formatDiagnostics(items: DiagnosticItem[]): string;

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
    /** Emit a UTF-8 Byte Order Mark (BOM) in the beginning of output files.
     * Defaults to `false`. */
    emitBOM?: boolean;
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
    /** Enables experimental support for ES decorators. Defaults to `false`. */
    experimentalDecorators?: boolean;
    /** Emit a single file with source maps instead of having a separate file.
     * Defaults to `false`. */
    inlineSourceMap?: boolean;
    /** Emit the source alongside the source maps within a single file; requires
     * `inlineSourceMap` or `sourceMap` to be set. Defaults to `false`. */
    inlineSources?: boolean;
    /** Perform additional checks to ensure that transpile only would be safe.
     * Defaults to `false`. */
    isolatedModules?: boolean;
    /** Support JSX in `.tsx` files: `"react"`, `"preserve"`, `"react-native"`.
     * Defaults to `"react"`. */
    jsx?: "react" | "preserve" | "react-native";
    /** Specify the JSX factory function to use when targeting react JSX emit,
     * e.g. `React.createElement` or `h`. Defaults to `React.createElement`. */
    jsxFactory?: string;
    /** Resolve keyof to string valued property names only (no numbers or
     * symbols). Defaults to `false`. */
    keyofStringsOnly?: string;
    /** Emit class fields with ECMAScript-standard semantics. Defaults to `false`.
     * Does not apply to `"esnext"` target. */
    useDefineForClassFields?: boolean;
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
    /** Do not add triple-slash references or module import targets to the list of
     * compiled files. Defaults to `false`. */
    noResolve?: boolean;
    /** Disable strict checking of generic signatures in function types. Defaults
     * to `false`. */
    noStrictGenericChecks?: boolean;
    /** Report errors on unused locals. Defaults to `false`. */
    noUnusedLocals?: boolean;
    /** Report errors on unused parameters. Defaults to `false`. */
    noUnusedParameters?: boolean;
    /** Redirect output structure to the directory. This only impacts
     * `Deno.compile` and only changes the emitted file names. Defaults to
     * `undefined`. */
    outDir?: string;
    /** List of path mapping entries for module names to locations relative to the
     * `baseUrl`. Defaults to `undefined`. */
    paths?: Record<string, string[]>;
    /** Do not erase const enum declarations in generated code. Defaults to
     * `false`. */
    preserveConstEnums?: boolean;
    /** Remove all comments except copy-right header comments beginning with
     * `/*!`. Defaults to `true`. */
    removeComments?: boolean;
    /** Include modules imported with `.json` extension. Defaults to `true`. */
    resolveJsonModule?: boolean;
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
  }

  /** **UNSTABLE**: new API, yet to be vetted.
   *
   * The results of a transpile only command, where the `source` contains the
   * emitted source, and `map` optionally contains the source map. */
  export interface TranspileOnlyResult {
    source: string;
    map?: string;
  }

  /** **UNSTABLE**: new API, yet to be vetted.
   *
   * Takes a set of TypeScript sources and resolves to a map where the key was
   * the original file name provided in sources and the result contains the
   * `source` and optionally the `map` from the transpile operation. This does no
   * type checking and validation, it effectively "strips" the types from the
   * file.
   *
   * ```ts
   * const results =  await Deno.transpileOnly({
   *   "foo.ts": `const foo: string = "foo";`
   * });
   * ```
   *
   * @param sources A map where the key is the filename and the value is the text
   *                to transpile. The filename is only used in the transpile and
   *                not resolved, for example to fill in the source name in the
   *                source map.
   * @param options An option object of options to send to the compiler. This is
   *                a subset of ts.CompilerOptions which can be supported by Deno.
   *                Many of the options related to type checking and emitting
   *                type declaration files will have no impact on the output.
   */
  export function transpileOnly(
    sources: Record<string, string>,
    options?: CompilerOptions,
  ): Promise<Record<string, TranspileOnlyResult>>;
  
   /** **UNSTABLE**: new API, yet to be vetted.
   * Returns the AST for the provided source file.
   * The extension of the module name will be used to determine the media type of the module.
   *
   * ```ts
   * const ast = await Deno.ast("foo.ts");
   * ```
   *
   * @param source  A source file to be parsed. The extension of the key will determine
   *                the media type of the file when processing.
   */
  export function ast(
    source: string,
  ): Promise<Program>;
  
  /** **UNSTABLE**: new API, yet to be vetted.
   *
   * Takes a root module name, and optionally a record set of sources. Resolves
   * with a compiled set of modules and possibly diagnostics if the compiler
   * encountered any issues. If just a root name is provided, the modules
   * will be resolved as if the root module had been passed on the command line.
   *
   * If sources are passed, all modules will be resolved out of this object, where
   * the key is the module name and the value is the content. The extension of
   * the module name will be used to determine the media type of the module.
   *
   * ```ts
   * const [ maybeDiagnostics1, output1 ] = await Deno.compile("foo.ts");
   *
   * const [ maybeDiagnostics2, output2 ] = await Deno.compile("/foo.ts", {
   *   "/foo.ts": `export * from "./bar.ts";`,
   *   "/bar.ts": `export const bar = "bar";`
   * });
   * ```
   *
   * @param rootName The root name of the module which will be used as the
   *                 "starting point". If no `sources` is specified, Deno will
   *                 resolve the module externally as if the `rootName` had been
   *                 specified on the command line.
   * @param sources An optional key/value map of sources to be used when resolving
   *                modules, where the key is the module name, and the value is
   *                the source content. The extension of the key will determine
   *                the media type of the file when processing. If supplied,
   *                Deno will not attempt to resolve any modules externally.
   * @param options An optional object of options to send to the compiler. This is
   *                a subset of ts.CompilerOptions which can be supported by Deno.
   */
  export function compile(
    rootName: string,
    sources?: Record<string, string>,
    options?: CompilerOptions,
  ): Promise<[DiagnosticItem[] | undefined, Record<string, string>]>;

  /** **UNSTABLE**: new API, yet to be vetted.
   *
   * `bundle()` is part the compiler API.  A full description of this functionality
   * can be found in the [manual](https://deno.land/manual/runtime/compiler_apis#denobundle).
   *
   * Takes a root module name, and optionally a record set of sources. Resolves
   * with a single JavaScript string (and bundle diagnostics if issues arise with
   * the bundling) that is like the output of a `deno bundle` command. If just
   * a root name is provided, the modules will be resolved as if the root module
   * had been passed on the command line.
   *
   * If sources are passed, all modules will be resolved out of this object, where
   * the key is the module name and the value is the content. The extension of the
   * module name will be used to determine the media type of the module.
   *
   * ```ts
   * // equivalent to "deno bundle foo.ts" from the command line
   * const [ maybeDiagnostics1, output1 ] = await Deno.bundle("foo.ts");
   *
   * const [ maybeDiagnostics2, output2 ] = await Deno.bundle("/foo.ts", {
   *   "/foo.ts": `export * from "./bar.ts";`,
   *   "/bar.ts": `export const bar = "bar";`
   * });
   * ```
   *
   * @param rootName The root name of the module which will be used as the
   *                 "starting point". If no `sources` is specified, Deno will
   *                 resolve the module externally as if the `rootName` had been
   *                 specified on the command line.
   * @param sources An optional key/value map of sources to be used when resolving
   *                modules, where the key is the module name, and the value is
   *                the source content. The extension of the key will determine
   *                the media type of the file when processing. If supplied,
   *                Deno will not attempt to resolve any modules externally.
   * @param options An optional object of options to send to the compiler. This is
   *                a subset of ts.CompilerOptions which can be supported by Deno.
   */
  export function bundle(
    rootName: string,
    sources?: Record<string, string>,
    options?: CompilerOptions,
  ): Promise<[DiagnosticItem[] | undefined, string]>;

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

  /** **UNSTABLE**: new API, yet to be vetted
   *
   * Set TTY to be under raw mode or not. In raw mode, characters are read and
   * returned as is, without being processed. All special processing of
   * characters by the terminal is disabled, including echoing input characters.
   * Reading from a TTY device in raw mode is faster than reading from a TTY
   * device in canonical mode.
   *
   * ```ts
   * Deno.setRaw(myTTY.rid, true);
   * ```
   */
  export function setRaw(rid: number, mode: boolean): void;

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

  /** **UNSTABLE**: Under consideration to remove `ShutdownMode` entirely.
   *
   * Corresponds to `SHUT_RD`, `SHUT_WR`, `SHUT_RDWR` on POSIX-like systems.
   *
   * See: http://man7.org/linux/man-pages/man2/shutdown.2.html */
  export enum ShutdownMode {
    Read = 0,
    Write,
    ReadWrite, // TODO(ry) panics on ReadWrite.
  }

  /** **UNSTABLE**: Both the `how` parameter and `ShutdownMode` enum are under
   * consideration for removal.
   *
   * Shutdown socket send and receive operations.
   *
   * Matches behavior of POSIX shutdown(3).
   *
   * ```ts
   * const listener = Deno.listen({ port: 80 });
   * const conn = await listener.accept();
   * Deno.shutdown(conn.rid, Deno.ShutdownMode.Write);
   * ```
   */
  export function shutdown(rid: number, how: ShutdownMode): Promise<void>;

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
   *        cmd: ["python", "-c", "from time import sleep; sleep(10000)"]
   *      });
   *
   *      Deno.kill(p.pid, Deno.Signal.SIGINT);
   *
   * Requires `allow-run` permission. */
  export function kill(pid: number, signo: number): void;

  /** The name of a "powerful feature" which needs permission.
   *
   * See: https://w3c.github.io/permissions/#permission-registry
   *
   * Note that the definition of `PermissionName` in the above spec is swapped
   * out for a set of Deno permissions which are not web-compatible. */
  export type PermissionName =
    | "run"
    | "read"
    | "write"
    | "net"
    | "env"
    | "plugin"
    | "hrtime";

  /** The current status of the permission.
   *
   * See: https://w3c.github.io/permissions/#status-of-a-permission */
  export type PermissionState = "granted" | "denied" | "prompt";

  export interface RunPermissionDescriptor {
    name: "run";
  }

  export interface ReadPermissionDescriptor {
    name: "read";
    path?: string;
  }

  export interface WritePermissionDescriptor {
    name: "write";
    path?: string;
  }

  export interface NetPermissionDescriptor {
    name: "net";
    /** Optional url associated with this descriptor.
     *
     * If specified: must be a valid url. Expected format: <scheme>://<host_or_ip>[:port][/path]
     * If the scheme is unknown, callers should specify some scheme, such as x:// na:// unknown://
     *
     * See: https://www.iana.org/assignments/uri-schemes/uri-schemes.xhtml */
    url?: string;
  }

  export interface EnvPermissionDescriptor {
    name: "env";
  }

  export interface PluginPermissionDescriptor {
    name: "plugin";
  }

  export interface HrtimePermissionDescriptor {
    name: "hrtime";
  }

  /** Permission descriptors which define a permission and can be queried,
   * requested, or revoked.
   *
   * See: https://w3c.github.io/permissions/#permission-descriptor */
  export type PermissionDescriptor =
    | RunPermissionDescriptor
    | ReadPermissionDescriptor
    | WritePermissionDescriptor
    | NetPermissionDescriptor
    | EnvPermissionDescriptor
    | PluginPermissionDescriptor
    | HrtimePermissionDescriptor;

  export class Permissions {
    /** Resolves to the current status of a permission.
     *
     * ```ts
     * const status = await Deno.permissions.query({ name: "read", path: "/etc" });
     * if (status.state === "granted") {
     *   data = await Deno.readFile("/etc/passwd");
     * }
     * ```
     */
    query(desc: PermissionDescriptor): Promise<PermissionStatus>;

    /** Revokes a permission, and resolves to the state of the permission.
     *
     *       const status = await Deno.permissions.revoke({ name: "run" });
     *       assert(status.state !== "granted")
     */
    revoke(desc: PermissionDescriptor): Promise<PermissionStatus>;

    /** Requests the permission, and resolves to the state of the permission.
     *
     * ```ts
     * const status = await Deno.permissions.request({ name: "env" });
     * if (status.state === "granted") {
     *   console.log(Deno.dir("home");
     * } else {
     *   console.log("'env' permission is denied.");
     * }
     * ```
     */
    request(desc: PermissionDescriptor): Promise<PermissionStatus>;
  }

  /** **UNSTABLE**: Under consideration to move to `navigator.permissions` to
   * match web API. It could look like `navigator.permissions.query({ name: Deno.symbols.read })`.
   */
  export const permissions: Permissions;

  /** see: https://w3c.github.io/permissions/#permissionstatus */
  export class PermissionStatus {
    state: PermissionState;
    constructor(state: PermissionState);
  }

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

  /** **UNSTABLE**: The URL of the file that was originally executed from the command-line. */
  export const mainModule: string;

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

  /* **UNSTABLE**: New API, yet to be vetted.
   * Synchronously flushes any pending data operations of the given file stream to disk.
   *  ```ts
   * const file = Deno.openSync("my_file.txt", { read: true, write: true, create: true });
   * Deno.writeSync(file.rid, new TextEncoder().encode("Hello World"));
   * Deno.fdatasyncSync(file.rid);
   * console.log(new TextDecoder().decode(Deno.readFileSync("my_file.txt"))); // Hello World
   * ```
   */
  export function fdatasyncSync(rid: number): void;

  /** **UNSTABLE**: New API, yet to be vetted.
   * Flushes any pending data operations of the given file stream to disk.
   *  ```ts
   * const file = await Deno.open("my_file.txt", { read: true, write: true, create: true });
   * await Deno.write(file.rid, new TextEncoder().encode("Hello World"));
   * await Deno.fdatasync(file.rid);
   * console.log(new TextDecoder().decode(await Deno.readFile("my_file.txt"))); // Hello World
   * ```
   */
  export function fdatasync(rid: number): Promise<void>;

  /** **UNSTABLE**: New API, yet to be vetted.
   * Synchronously flushes any pending data and metadata operations of the given file stream to disk.
   *  ```ts
   * const file = Deno.openSync("my_file.txt", { read: true, write: true, create: true });
   * Deno.writeSync(file.rid, new TextEncoder().encode("Hello World"));
   * Deno.ftruncateSync(file.rid, 1);
   * Deno.fsyncSync(file.rid);
   * console.log(new TextDecoder().decode(Deno.readFileSync("my_file.txt"))); // H
   * ```
   */
  export function fsyncSync(rid: number): void;

  /** **UNSTABLE**: New API, yet to be vetted.
   * Flushes any pending data and metadata operations of the given file stream to disk.
   *  ```ts
   * const file = await Deno.open("my_file.txt", { read: true, write: true, create: true });
   * await Deno.write(file.rid, new TextEncoder().encode("Hello World"));
   * await Deno.ftruncate(file.rid, 1);
   * await Deno.fsync(file.rid);
   * console.log(new TextDecoder().decode(await Deno.readFile("my_file.txt"))); // H
   * ```
   */
  export function fsync(rid: number): Promise<void>;

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
}

// --- AST Nodes ---

export interface Span {
  start: number;
  end: number;
  ctxt: number;
}

export interface Node {
  type: string;
}

export interface HasSpan {
  span: Span;
}

export interface HasDecorator {
  decorators?: Decorator[];
}

export interface Class extends HasSpan, HasDecorator {
  body: ClassMember[];

  superClass?: Expression;

  is_abstract: boolean;

  typeParams: TsTypeParameterDeclaration;

  superTypeParams?: TsTypeParameterInstantiation;

  implements: TsExpressionWithTypeArguments[];
}

export type ClassMember =
  | Constructor
  | ClassMethod
  | PrivateMethod
  | ClassProperty
  | PrivateProperty
  | TsIndexSignature;

export interface ClassPropertyBase extends Node, HasSpan, HasDecorator {
  value?: Expression;

  typeAnnotation?: TsTypeAnnotation;

  is_static: boolean;

  computed: boolean;

  accessibility?: Accessibility;

  /// Typescript extension.
  is_abstract: boolean;

  is_optional: boolean;

  readonly: boolean;

  definite: boolean;
}

export interface ClassProperty extends ClassPropertyBase {
  type: "ClassProperty";

  key: Expression;
}

export interface PrivateProperty extends ClassPropertyBase {
  type: "PrivateProperty";

  key: PrivateName;
}

export interface Param extends Node, HasSpan, HasDecorator {
  type: 'Parameter'
  pat: Pattern
}

export interface Constructor extends Node, HasSpan {
  type: "Constructor";

  key: PropertyName;

  params: (Param | TsParameterProperty)[];

  body: BlockStatement;

  accessibility?: Accessibility;

  is_optional: boolean;
}

export interface ClassMethodBase extends Node, HasSpan {
  function: Fn;

  kind: MethodKind;

  is_static: boolean;

  accessibility?: Accessibility;

  is_abstract: boolean;

  is_optional: boolean;
}

export interface ClassMethod extends ClassMethodBase {
  type: "ClassMethod";

  key: PropertyName;
}

export interface PrivateMethod extends ClassMethodBase {
  type: "PrivateMethod";

  key: PrivateName;
}

export interface Decorator extends Node, HasSpan {
  type: "Decorator";

  expression: Expression;
}

export type MethodKind = "method" | "setter" | "getter";

export type Declaration =
  | ClassDeclaration
  | FunctionDeclaration
  | VariableDeclaration
  | TsInterfaceDeclaration
  | TsTypeAliasDeclaration
  | TsEnumDeclaration
  | TsModuleDeclaration;

export interface FunctionDeclaration extends Fn {
  type: "FunctionDeclaration";

  ident: Identifier;

  declare: boolean;
}

export interface ClassDeclaration extends Class, Node {
  type: "ClassDeclaration";

  identifier: Identifier;

  declare: boolean;
}

export interface VariableDeclaration extends Node, HasSpan {
  type: "VariableDeclaration";

  kind: VariableDeclarationKind;

  declare: boolean;

  declarations: VariableDeclarator[];
}

export type VariableDeclarationKind = "get" | "let" | "const";

export interface VariableDeclarator extends Node, HasSpan {
  type: "VariableDeclarator";

  id: Pattern;

  /// Initialization expresion.
  init?: Expression;

  /// Typescript only
  definite: boolean;
}

export type Expression =
  | ThisExpression
  | ArrayExpression
  | ObjectExpression
  | FunctionExpression
  | UnaryExpression
  | UpdateExpression
  | BinaryExpression
  | AssignmentExpression
  | MemberExpression
  | ConditionalExpression
  | CallExpression
  | NewExpression
  | SequenceExpression
  | Identifier
  | Literal
  | TemplateLiteral
  | TaggedTemplateExpression
  | ArrowFunctionExpression
  | ClassExpression
  | YieldExpression
  | MetaProperty
  | AwaitExpression
  | ParenthesisExpression
  | JSXMemberExpression
  | JSXNamespacedName
  | JSXEmptyExpression
  | JSXElement
  | JSXFragment
  | TsTypeAssertion
  | TsNonNullExpression
  | TsTypeCastExpression
  | TsAsExpression
  | PrivateName
  | OptionalChainingExpression
  | Invalid;

interface ExpressionBase extends Node, HasSpan { }

export interface OptionalChainingExpression extends ExpressionBase {
  type: "OptionalChainingExpression";
  /**
   * Call expression or member expression.
   */
  expr: Expression;
}

export interface ThisExpression extends ExpressionBase {
  type: "ThisExpression";
}

export interface ArrayExpression extends ExpressionBase {
  type: "ArrayExpression";

  elements: (Expression | SpreadElement | undefined)[];
}

export interface ObjectExpression extends ExpressionBase {
  type: "ObjectExpression";

  properties: (Property | SpreadElement)[];
}

export interface Argument {
  spread: Span;
  expression: Expression;
}

export type PropertOrSpread = Property | SpreadElement;

export interface SpreadElement extends Node {
  type: "SpreadElement";

  spread: Span;

  arguments: Expression;
}

export interface UnaryExpression extends ExpressionBase {
  type: "UnaryExpression";

  operator: UnaryOperator;

  argument: Expression;
}

export interface UpdateExpression extends ExpressionBase {
  type: "UpdateExpression";

  operator: UpdateOperator;

  prefix: boolean;

  argument: Expression;
}

export interface BinaryExpression extends ExpressionBase {
  type: "BinaryExpression";

  operator: BinaryOperator;

  left: Expression;

  right: Expression;
}

export interface FunctionExpression extends Fn, ExpressionBase {
  type: "FunctionExpression";

  identifier: Identifier;
}

export interface ClassExpression extends Class, ExpressionBase {
  type: "ClassExpression";

  identifier: Identifier;
}

export interface AssignmentExpression extends ExpressionBase {
  type: "AssignmentExpression";

  operator: AssignmentOperator;

  left: Pattern | Expression;

  right: Expression;
}

export interface MemberExpression extends ExpressionBase {
  type: "MemberExpression";

  object: Expression | Super;

  property: Expression;

  computed: boolean;
}

export interface ConditionalExpression extends ExpressionBase {
  type: "ConditionalExpression";

  test: Expression;

  consequent: Expression;

  alternate: Expression;
}

export interface Super extends Node, HasSpan {
  type: "Super";
}

export interface CallExpression extends ExpressionBase {
  type: "CallExpression";

  callee: Expression | Super;

  arguments: Argument[];

  typeArguments?: TsTypeParameterInstantiation;
}

export interface NewExpression extends ExpressionBase {
  type: "NewExpression";

  callee: Expression;

  arguments: Argument[];

  typeArguments?: TsTypeParameterInstantiation;
}

export interface SequenceExpression extends ExpressionBase {
  type: "SequenceExpression";

  expressions: Expression[];
}

export interface ArrowFunctionExpression extends ExpressionBase {
  type: "ArrowFunctionExpression";

  params: Pattern[];

  body: BlockStatement | Expression;

  async: boolean;

  generator: boolean;

  typeParameters?: TsTypeParameterDeclaration;

  returnType?: TsTypeAnnotation;
}

export interface YieldExpression extends ExpressionBase {
  type: "YieldExpression";

  argument?: Expression;

  delegate: boolean;
}

export interface MetaProperty extends Node {
  type: "MetaProperty";

  meta: Identifier;

  property: Identifier;
}

export interface AwaitExpression extends ExpressionBase {
  type: "AwaitExpression";

  argument: Expression;
}

export interface TplBase {
  expressions: Expression[];

  quasis: TemplateElement[];
}

export interface TemplateLiteral extends ExpressionBase, TplBase {
  type: "TemplateLiteral";
}

export interface TaggedTemplateExpression extends ExpressionBase, TplBase {
  type: "TaggedTemplateExpression";

  tag: Expression;

  typeParameters: TsTypeParameterInstantiation;
}

export interface TemplateElement extends ExpressionBase {
  type: "TemplateElement";

  tail: boolean;
  cooked: StringLiteral;
  raw: StringLiteral;
}

export interface ParenthesisExpression extends ExpressionBase {
  type: "ParenthesisExpression";

  expression: Expression;
}

export interface Fn extends HasSpan, HasDecorator {
  params: Param[];

  body: BlockStatement;

  generator: boolean;

  async: boolean;

  typeParameters?: TsTypeParameterDeclaration;

  returnType?: TsTypeAnnotation;
}

interface PatternBase {
  typeAnnotation?: TsTypeAnnotation;
}

export interface Identifier extends HasSpan, PatternBase {
  type: "Identifier";

  value: string;

  /// TypeScript only. Used in case of an optional parameter.
  optional: boolean;
}

export interface PrivateName extends ExpressionBase {
  type: "PrivateName";

  id: Identifier;
}

export type JSXObject = JSXMemberExpression | Identifier;

export interface JSXMemberExpression extends Node {
  type: "JSXMemberExpression";

  object: JSXObject;
  property: Identifier;
}

/**
 * XML-based namespace syntax:
 */
export interface JSXNamespacedName extends Node {
  type: "JSXNamespacedName";

  namespace: Identifier;
  name: Identifier;
}

export interface JSXEmptyExpression extends Node, HasSpan {
  type: "JSXEmptyExpression";
}

export interface JSXExpressionContainer extends Node {
  type: "JSXExpressionContainer";

  expression: JSXExpression;
}

export type JSXExpression = JSXEmptyExpression | Expression;

export interface JSXSpreadChild extends Node {
  type: "JSXSpreadChild";

  expression: Expression;
}

export type JSXElementName =
  | Identifier
  | JSXMemberExpression
  | JSXNamespacedName;

export interface JSXOpeningElement extends Node, HasSpan {
  type: "JSXOpeningElement";

  name: JSXElementName;

  attrs?: JSXAttributeOrSpread[];

  selfClosing: boolean;

  typeArguments?: TsTypeParameterInstantiation;
}

export type JSXAttributeOrSpread = JSXAttribute | SpreadElement;

export interface JSXClosingElement extends Node, HasSpan {
  type: "JSXClosingElement";

  name: JSXElementName;
}

export interface JSXAttribute extends Node, HasSpan {
  type: "JSXAttribute";

  name: JSXAttributeName;

  value?: JSXAttrValue;
}

export type JSXAttributeName = Identifier | JSXNamespacedName;

export type JSXAttrValue =
  | Literal
  | JSXExpressionContainer
  | JSXElement
  | JSXFragment;

export interface JSXText extends Node, HasSpan {
  type: "JSXText";

  value: string;
  raw: string;
}

export interface JSXElement extends Node, HasSpan {
  type: "JSXElement";

  opening: JSXOpeningElement;
  children: JSXElementChild[];
  closing?: JSXClosingElement;
}

export type JSXElementChild =
  | JSXText
  | JSXExpressionContainer
  | JSXSpreadChild
  | JSXElement
  | JSXFragment;

export interface JSXFragment extends Node, HasSpan {
  type: "JSXFragment";

  opening: JSXOpeningFragment;

  children: JSXElementChild[];

  closing: JSXClosingFragment;
}

export interface JSXOpeningFragment extends Node, HasSpan {
  type: "JSXOpeningFragment";
}

export interface JSXClosingFragment extends Node, HasSpan {
  type: "JSXClosingFragment";
}

export type Literal =
  | StringLiteral
  | BooleanLiteral
  | NullLiteral
  | NumericLiteral
  | RegExpLiteral
  | JSXText;

export interface StringLiteral extends Node, HasSpan {
  type: "StringLiteral";

  value: string;
  has_escape: boolean;
}

export interface BooleanLiteral extends Node, HasSpan {
  type: "BooleanLiteral";

  value: boolean;
}

export interface NullLiteral extends Node, HasSpan {
  type: "NullLiteral";
}

export interface RegExpLiteral extends Node, HasSpan {
  type: "RegExpLiteral";

  pattern: string;
  flags: string;
}

export interface NumericLiteral extends Node, HasSpan {
  type: "NumericLiteral";

  value: number;
}

export type ModuleDeclaration =
  | ImportDeclaration
  | ExportDeclaration
  | ExportNamedDeclaration
  | ExportDefaultDeclaration
  | ExportDefaultExpression
  | ExportAllDeclaration
  | TsImportEqualsDeclaration
  | TsExportAssignment
  | TsNamespaceExportDeclaration;

export interface ExportDefaultExpression extends Node, HasSpan {
  type: "ExportDefaultExpression";

  expression: Expression;
}

export interface ExportDeclaration extends Node, HasSpan {
  type: "ExportDeclaration";

  declaration: Declaration;
}

export interface ImportDeclaration extends Node, HasSpan {
  type: "ImportDeclaration";

  specifiers: ImporSpecifier[];

  source: StringLiteral;
}

export type ImporSpecifier =
  | ImportDefaultSpecifier
  | NamedImportSpecifier
  | ImportNamespaceSpecifier;

export interface ExportAllDeclaration extends Node, HasSpan {
  type: "ExportAllDeclaration";

  source: StringLiteral;
}

/**
 * - `export { foo } from 'mod'`
 * - `export { foo as bar } from 'mod'`
 */
export interface ExportNamedDeclaration extends Node, HasSpan {
  type: "ExportNamedDeclaration";

  specifiers: ExportSpecifier[];

  source?: StringLiteral;
}

export interface ExportDefaultDeclaration extends Node, HasSpan {
  type: "ExportDefaultDeclaration";

  decl: DefaultDecl;
}

export type DefaultDecl =
  | ClassExpression
  | FunctionExpression
  | TsInterfaceDeclaration;

export type ImportSpecifier =
  | NamedImportSpecifier
  | ImportDefaultSpecifier
  | ImportNamespaceSpecifier;

/**
 * e.g. `import foo from 'mod.js'`
 */
export interface ImportDefaultSpecifier extends Node, HasSpan {
  type: "ImportDefaultSpecifier";
  local: Identifier;
}

/**
 * e.g. `import * as foo from 'mod.js'`.
 */
export interface ImportNamespaceSpecifier extends Node, HasSpan {
  type: "ImportNamespaceSpecifier";

  local: Identifier;
}

/**
 * e.g. - `import { foo } from 'mod.js'`
 *
 * local = foo, imported = None
 *
 * e.g. `import { foo as bar } from 'mod.js'`
 *
 * local = bar, imported = Some(foo) for
 */
export interface NamedImportSpecifier extends Node, HasSpan {
  type: "ImportSpecifier";
  local: Identifier;
  imported: Identifier;
}

export type ExportSpecifier =
  | ExportNamespaceSpecifer
  | ExportDefaultSpecifier
  | NamedExportSpecifier;

/**
 * `export * as foo from 'src';`
 */
export interface ExportNamespaceSpecifer extends Node, HasSpan {
  type: "ExportNamespaceSpecifer";

  name: Identifier;
}

export interface ExportDefaultSpecifier extends Node, HasSpan {
  type: "ExportDefaultSpecifier";

  exported: Identifier;
}

export interface NamedExportSpecifier extends Node, HasSpan {
  type: "ExportSpecifier";

  orig: Identifier;
  /**
   * `Some(bar)` in `export { foo as bar }`
   */
  exported: Identifier;
}

interface HasInterpreter {
  /**
   * e.g. `/usr/bin/node` for `#!/usr/bin/node`
   */
  interpreter: string;
}

export type Program = Module | Script;

export interface Module extends Node, HasSpan, HasInterpreter {
  type: "Module";

  body: ModuleItem[];
}

export interface Script extends Node, HasSpan, HasInterpreter {
  type: "Script";

  body: Statement[];
}

export type ModuleItem = ModuleDeclaration | Statement;

export type BinaryOperator =
  | "=="
  | "!="
  | "==="
  | "!=="
  | "<"
  | "<="
  | ">"
  | ">="
  | "<<"
  | ">>"
  | ">>>"
  | "+"
  | "-"
  | "*"
  | "/"
  | "%"
  | "**"
  | "|"
  | "^"
  | "&"
  | "||"
  | "&&"
  | "in"
  | "instanceof"
  | "??";

export type AssignmentOperator =
  | "="
  | "+="
  | "-="
  | "*="
  | "/="
  | "%="
  | "**="
  | "<<="
  | ">>="
  | ">>>="
  | "|="
  | "^="
  | "&=";

export type UpdateOperator = "++" | "--";

export type UnaryOperator =
  | "-"
  | "+"
  | "!"
  | "~"
  | "typeof"
  | "void"
  | "delete";

export type Pattern =
  | Identifier
  | ArrayPattern
  | RestElement
  | ObjectPattern
  | AssignmentPattern
  | Invalid
  | Expression;

export interface ArrayPattern extends Node, HasSpan, PatternBase {
  type: "ArrayPattern";

  elements: (Pattern | undefined)[];
}

export interface ObjectPattern extends Node, HasSpan, PatternBase {
  type: "ObjectPattern";

  props: ObjectPatternProperty[];
}

export interface AssignmentPattern extends Node, HasSpan, PatternBase {
  type: "AssignmentPattern";

  left: Pattern;
  right: Expression;
}

export interface RestElement extends Node, HasSpan, PatternBase {
  type: "RestElement";

  rest: Span;

  argument: Pattern;
}

export type ObjectPatternProperty =
  | KeyValuePatternProperty
  | AssignmentPatternProperty
  | RestElement;

/**
 * `{key: value}`
 */
export interface KeyValuePatternProperty extends Node {
  type: "KeyValuePatternProperty";

  key: PropertyName;
  value: Pattern;
}

/**
 * `{key}` or `{key = value}`
 */
export interface AssignmentPatternProperty extends Node, HasSpan {
  type: "AssignmentPatternProperty";

  key: Identifier;
  value?: Expression;
}

/** Identifier is `a` in `{ a, }` */
export type Property =
  | Identifier
  | KeyValueProperty
  | AssignmentProperty
  | GetterProperty
  | SetterProperty
  | MethodProperty;

interface PropBase extends Node {
  key: PropertyName;
}

export interface KeyValueProperty extends PropBase {
  type: "KeyValueProperty";

  value: Expression;
}

export interface AssignmentProperty extends Node {
  type: "AssignmentProperty";

  key: Identifier;
  value: Expression;
}

export interface GetterProperty extends PropBase, HasSpan {
  type: "GetterProperty";

  typeAnnotation?: TsTypeAnnotation;

  body: BlockStatement;
}

export interface SetterProperty extends PropBase, HasSpan {
  type: "SetterProperty";

  param: Pattern;
  body: BlockStatement;
}

export interface MethodProperty extends PropBase, Fn {
  type: "MethodProperty";
}

export type PropertyName =
  | Identifier
  | StringLiteral
  | NumericLiteral
  | ComputedPropName;

export interface ComputedPropName extends Node, HasSpan {
  type: "Computed";
  expression: Expression;
}

export interface BlockStatement extends Node, HasSpan {
  type: "BlockStatement";

  stmts: Statement[];
}

export interface ExpressionStatement extends Node, HasSpan {
  type: "ExpressionStatement";
  expression: Expression;
}

export type Statement =
  | ExpressionStatement
  | BlockStatement
  | EmptyStatement
  | DebuggerStatement
  | WithStatement
  | ReturnStatement
  | LabeledStatement
  | BreakStatement
  | ContinueStatement
  | IfStatement
  | SwitchStatement
  | ThrowStatement
  | TryStatement
  | WhileStatement
  | DoWhileStatement
  | ForStatement
  | ForInStatement
  | ForOfStatement
  | Declaration;

export interface EmptyStatement extends Node, HasSpan {
  type: "EmptyStatement";
}

export interface DebuggerStatement extends Node, HasSpan {
  type: "DebuggerStatement";
}

export interface WithStatement extends Node, HasSpan {
  type: "WithStatement";

  object: Expression;
  body: Statement;
}

export interface ReturnStatement extends Node, HasSpan {
  type: "ReturnStatement";

  argument: Expression;
}

export interface LabeledStatement extends Node, HasSpan {
  type: "LabeledStatement";

  label: Identifier;
  body: Statement;
}

export interface BreakStatement extends Node, HasSpan {
  type: "BreakStatement";

  label: Identifier;
}

export interface ContinueStatement extends Node, HasSpan {
  type: "ContinueStatement";

  label: Identifier;
}

export interface IfStatement extends Node, HasSpan {
  type: "IfStatement";

  test: Expression;
  consequent: Statement;
  alternate?: Statement;
}

export interface SwitchStatement extends Node, HasSpan {
  type: "SwitchStatement";

  discriminant: Expression;
  cases: SwitchCase[];
}

export interface ThrowStatement extends Node, HasSpan {
  type: "ThrowStatement";

  argument: Expression;
}

export interface TryStatement extends Node, HasSpan {
  type: "TryStatement";

  block: BlockStatement;
  handler?: CatchClause;
  finalizer: BlockStatement;
}

export interface WhileStatement extends Node, HasSpan {
  type: "WhileStatement";

  test: Expression;
  body: Statement;
}

export interface DoWhileStatement extends Node, HasSpan {
  type: "DoWhileStatement";

  test: Expression;
  body: Statement;
}

export interface ForStatement extends Node, HasSpan {
  type: "ForStatement";

  init?: VariableDeclaration | Expression;
  test?: Expression;
  update?: Expression;
  body: Statement;
}

export interface ForInStatement extends Node, HasSpan {
  type: "ForInStatement";

  left: VariableDeclaration | Pattern;
  right: Expression;
  body: Statement;
}

export interface ForOfStatement extends Node, HasSpan {
  type: "ForOfStatement";

  /**
   *  Span of the await token.
   *
   *  es2018 for-await-of statements, e.g., `for await (const x of xs) {`
   */
  await: Span;
  left: VariableDeclaration | Pattern;
  right: Expression;
  body: Statement;
}

export interface SwitchCase extends Node, HasSpan {
  type: "SwitchCase";

  /**
   * Undefined for default case
   */
  test?: Expression;
  consequent: Statement[];
}

export interface CatchClause extends Node, HasSpan {
  type: "CatchClause";

  /**
   * The param is `undefined` if the catch binding is omitted. E.g., `try { foo() } catch {}`
   */
  param: Pattern;
  body: BlockStatement;
}

export interface TsTypeAnnotation extends Node, HasSpan {
  type: "TsTypeAnnotation";

  typeAnnotation: TsType;
}

export interface TsTypeParameterDeclaration extends Node, HasSpan {
  type: "TsTypeParameterDeclaration";

  parameters: TsTypeParameter[];
}

export interface TsTypeParameter extends Node, HasSpan {
  type: "TsTypeParameter";

  name: Identifier;
  constraint: TsType;
  default: TsType;
}

export interface TsTypeParameterInstantiation extends Node, HasSpan {
  type: "TsTypeParameterInstantiation";

  params: TsType[];
}

export interface TsTypeCastExpression extends Node, HasSpan {
  type: "TsTypeCastExpression";

  expression: Expression;
  typeAnnotation: TsTypeAnnotation;
}

export interface TsParameterProperty extends Node, HasSpan, HasDecorator {
  type: "TsParameterProperty";

  accessibility?: Accessibility;
  readonly: boolean;
  param: TsParameterPropertyParameter;
}

export type TsParameterPropertyParameter = Identifier | AssignmentPattern;

export interface TsQualifiedName extends Node {
  type: "TsQualifiedName";

  left: TsEntityName;
  right: Identifier;
}

export type TsEntityName = TsQualifiedName | Identifier;

export type TsSignatureDeclaration =
  | TsCallSignatureDeclaration
  | TsConstructSignatureDeclaration
  | TsMethodSignature
  | TsFunctionType
  | TsConstructorType;

export type TsTypeElement =
  | TsCallSignatureDeclaration
  | TsConstructSignatureDeclaration
  | TsPropertySignature
  | TsMethodSignature
  | TsIndexSignature;

export interface TsCallSignatureDeclaration extends Node, HasSpan {
  type: "TsCallSignatureDeclaration";

  params: TsFnParameter[];
  typeAnnotation: TsTypeAnnotation;
  typeParams: TsTypeParameterDeclaration;
}

export interface TsConstructSignatureDeclaration extends Node, HasSpan {
  type: "TsConstructSignatureDeclaration";

  params: TsFnParameter[];
  typeAnnotation: TsTypeAnnotation;
  typeParams: TsTypeParameterDeclaration;
}

export interface TsPropertySignature extends Node, HasSpan {
  type: "TsPropertySignature";

  readonly: boolean;
  key: Expression;
  computed: boolean;
  optional: boolean;

  init: Expression;
  params: TsFnParameter[];

  typeAnnotation?: TsTypeAnnotation;
  typeParams: TsTypeParameterDeclaration;
}

export interface TsMethodSignature extends Node, HasSpan {
  type: "TsMethodSignature";

  readonly: boolean;
  key: Expression;
  computed: boolean;
  optional: boolean;
  params: TsFnParameter[];

  typeAnnotation: TsTypeAnnotation;
  typeParams: TsTypeParameterDeclaration;
}

export interface TsIndexSignature extends Node, HasSpan {
  type: "TsIndexSignature";

  readonly: boolean;
  params: TsFnParameter[];

  typeAnnotation?: TsTypeAnnotation;
}

export type TsType =
  | TsKeywordType
  | TsThisType
  | TsFnOrConstructorType
  | TsTypeReference
  | TsTypeQuery
  | TsTypeLiteral
  | TsArrayType
  | TsTupleType
  | TsOptionalType
  | TsRestType
  | TsUnionOrIntersectionType
  | TsConditionalType
  | TsInferType
  | TsParenthesizedType
  | TsTypeOperator
  | TsIndexedAccessType
  | TsMappedType
  | TsLiteralType
  | TsImportType
  | TsTypePredicate;

export type TsFnOrConstructorType = TsFunctionType | TsConstructorType;

export interface TsKeywordType extends Node, HasSpan {
  type: "TsKeywordType";

  kind: TsKeywordTypeKind;
}

export type TsKeywordTypeKind =
  | "any"
  | "unknown"
  | "number"
  | "object"
  | "boolean"
  | "bigint"
  | "string"
  | "symbol"
  | "void"
  | "undefined"
  | "null"
  | "never";

export interface TsThisType extends Node, HasSpan {
  type: "TsThisType";
}

export type TsFnParameter = Identifier | RestElement | ObjectPattern;

export interface TsFunctionType extends Node, HasSpan {
  type: "TsFunctionType";

  typeParams: TsTypeParameterDeclaration;
  typeAnnotation: TsTypeAnnotation;
}

export interface TsConstructorType extends Node, HasSpan {
  type: "TsConstructorType";

  params: TsFnParameter[];

  typeParams: TsTypeParameterDeclaration;
  typeAnnotation: TsTypeAnnotation;
}

export interface TsTypeReference extends Node, HasSpan {
  type: "TsTypeReference";

  typeName: TsEntityName;
  typeParams: TsTypeParameterInstantiation;
}

export interface TsTypePredicate extends Node, HasSpan {
  type: "TsTypePredicate";

  asserts: boolean;

  paramName: TsThisTypeOrIdent;
  typeAnnotation: TsTypeAnnotation;
}

export type TsThisTypeOrIdent = TsThisType | Identifier;

export interface TsImportType extends Node, HasSpan {
  argument: StringLiteral;
  qualifier?: TsEntityName;
  typeArguments?: TsTypeParameterInstantiation;
}

/**
 * `typeof` operator
 */
export interface TsTypeQuery extends Node, HasSpan {
  type: "TsTypeQuery";

  exprName: TsTypeQueryExpr;
}

export type TsTypeQueryExpr = TsEntityName | TsImportType;

export interface TsTypeLiteral extends Node, HasSpan {
  type: "TsTypeLiteral";

  members: TsTypeElement[];
}

export interface TsArrayType extends Node, HasSpan {
  type: "TsArrayType";

  elemType: TsType;
}

export interface TsTupleType extends Node, HasSpan {
  type: "TsTupleType";

  elemTypes: TsType[];
}

export interface TsOptionalType extends Node, HasSpan {
  type: "TsOptionalType";

  typeAnnotation: TsType;
}

export interface TsRestType extends Node, HasSpan {
  type: "TsRestType";

  typeAnnotation: TsType;
}

export type TsUnionOrIntersectionType = TsUnionType | TsIntersectionType;

export interface TsUnionType extends Node, HasSpan {
  type: "TsUnionType";

  types: TsType[];
}

export interface TsIntersectionType extends Node, HasSpan {
  type: "TsIntersectionType";

  types: TsType[];
}

export interface TsConditionalType extends Node, HasSpan {
  type: "TsConditionalType";

  checkType: TsType;
  extendsType: TsType;
  trueType: TsType;
  falseType: TsType;
}

export interface TsInferType extends Node, HasSpan {
  type: "TsInferType";

  typeParam: TsTypeParameter;
}

export interface TsParenthesizedType extends Node, HasSpan {
  type: "TsParenthesizedType";

  typeAnnotation: TsType;
}

export interface TsTypeOperator extends Node, HasSpan {
  type: "TsTypeOperator";

  op: TsTypeOperatorOp;
  typeAnnotation: TsType;
}

export type TsTypeOperatorOp = "keyof" | "unique";

export interface TsIndexedAccessType extends Node, HasSpan {
  type: "TsIndexedAccessType";

  objectType: TsType;
  indexType: TsType;
}

export type TruePlusMinus = true | "+" | "-";

export interface TsMappedType extends Node, HasSpan {
  type: "TsMappedType";

  readonly: TruePlusMinus;
  typeParam: TsTypeParameter;
  optional: TruePlusMinus;
  typeAnnotation: TsType;
}

export interface TsLiteralType extends Node, HasSpan {
  type: "TsLiteralType";

  literal: TsLiteral;
}

export type TsLiteral = NumericLiteral | StringLiteral | BooleanLiteral | TemplateLiteral;

// // ================
// // TypeScript declarations
// // ================

export interface TsInterfaceDeclaration extends Node, HasSpan {
  type: "TsInterfaceDeclaration";

  id: Identifier;
  declare: boolean;
  typeParams?: TsTypeParameterDeclaration;
  extends: TsExpressionWithTypeArguments[];
  body: TsInterfaceBody;
}

export interface TsInterfaceBody extends Node, HasSpan {
  type: "TsInterfaceBody";

  body: TsTypeElement[];
}

export interface TsExpressionWithTypeArguments extends Node, HasSpan {
  type: "TsExpressionWithTypeArguments";

  expression: TsEntityName;
  typeArguments?: TsTypeParameterInstantiation;
}

export interface TsTypeAliasDeclaration extends Node, HasSpan {
  type: "TsTypeAliasDeclaration";

  declare: boolean;
  id: Identifier;
  typeParams?: TsTypeParameterDeclaration;
  typeAnnotation: TsType;
}

export interface TsEnumDeclaration extends Node, HasSpan {
  type: "TsEnumDeclaration";

  declare: boolean;
  is_const: boolean;
  id: Identifier;
  member: TsEnumMember[];
}

export interface TsEnumMember extends Node, HasSpan {
  type: "TsEnumMember";

  id: TsEnumMemberId;
  init?: Expression;
}

export type TsEnumMemberId = Identifier | StringLiteral;

export interface TsModuleDeclaration extends Node, HasSpan {
  type: "TsModuleDeclaration";

  declare: boolean;
  global: boolean;
  id: TsModuleName;
  body?: TsNamespaceBody;
}

/**
 * `namespace A.B { }` is a namespace named `A` with another TsNamespaceDecl as its body.
 */
export type TsNamespaceBody = TsModuleBlock | TsNamespaceDeclaration;

export interface TsModuleBlock extends Node, HasSpan {
  type: "TsModuleBlock";

  body: ModuleItem[];
}

export interface TsNamespaceDeclaration extends Node, HasSpan {
  type: "TsNamespaceDeclaration";

  declare: boolean;
  global: boolean;
  id: Identifier;
  body: TsNamespaceBody;
}

export type TsModuleName = Identifier | StringLiteral;

export interface TsImportEqualsDeclaration extends Node, HasSpan {
  type: "TsImportEqualsDeclaration";

  declare: boolean;
  is_export: boolean;
  id: Identifier;
  moduleRef: TsModuleReference;
}

export type TsModuleReference = TsEntityName | TsExternalModuleReference;

export interface TsExternalModuleReference extends Node, HasSpan {
  type: "TsExternalModuleReference";

  expression: Expression;
}

export interface TsExportAssignment extends Node, HasSpan {
  type: "TsExportAssignment";

  expression: Expression;
}

export interface TsNamespaceExportDeclaration extends Node, HasSpan {
  type: "TsNamespaceExportDeclaration";

  id: Identifier;
}

export interface TsAsExpression extends ExpressionBase {
  type: "TsAsExpression";

  expression: Expression;
  typeAnnotation: TsType;
}

export interface TsTypeAssertion extends ExpressionBase {
  type: "TsTypeAssertion";

  expression: Expression;
  typeAnnotation: TsType;
}

export interface TsNonNullExpression extends ExpressionBase {
  type: "TsNonNullExpression";

  expression: Expression;
}

export type Accessibility = "public" | "protected" | "private";

export interface Invalid extends Node, HasSpan {
  type: "Invalid";
}
