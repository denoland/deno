// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

/// <reference no-default-lib="true" />

declare namespace Deno {
  const __unstable: true;

  /**
   * **UNSTABLE**: New API, yet to be vetted.  This API is under consideration to
   * determine if permissions are required to call it.
   *
   * Retrieve the process umask.  If `mask` is provided, sets the process umask.
   * This call always returns what the umask was before the call.
   *
   *        console.log(Deno.umask());  // e.g. 18 (0o022)
   *        const prevUmaskValue = Deno.umask(0o077);  // e.g. 18 (0o022)
   *        console.log(Deno.umask());  // e.g. 63 (0o077)
   *
   * NOTE:  This API is not implemented on Windows
   */
  export function umask(mask?: number): number;

  /** Synchronously creates `newpath` as a hard link to `oldpath`.
   *
   *       Deno.linkSync("old/name", "new/name");
   *
   * Requires `allow-read` and `allow-write` permissions. */
  export function linkSync(oldpath: string, newpath: string): void;

  /** Creates `newpath` as a hard link to `oldpath`.
   *
   *  **UNSTABLE**: needs security review.
   *
   *       await Deno.link("old/name", "new/name");
   *
   * Requires `allow-read` and `allow-write` permissions. */
  export function link(oldpath: string, newpath: string): Promise<void>;

  /** **UNSTABLE**: `type` argument type may be changed to `"dir" | "file"`.
   *
   *  **UNSTABLE**: needs security review.
   *
   * Creates `newpath` as a symbolic link to `oldpath`.
   *
   * The type argument can be set to `dir` or `file`. This argument is only
   * available on Windows and ignored on other platforms.
   *
   * NOTE: This function is not yet implemented on Windows.
   *
   *       Deno.symlinkSync("old/name", "new/name");
   *
   * Requires `allow-read` and `allow-write` permissions. */
  export function symlinkSync(
    oldpath: string,
    newpath: string,
    type?: string
  ): void;

  /** **UNSTABLE**: `type` argument may be changed to `"dir" | "file"`
   *
   *  **UNSTABLE**: needs security review.
   *
   * Creates `newpath` as a symbolic link to `oldpath`.
   *
   * The type argument can be set to `dir` or `file`. This argument is only
   * available on Windows and ignored on other platforms.
   *
   * NOTE: This function is not yet implemented on Windows.
   *
   *       await Deno.symlink("old/name", "new/name");
   *
   * Requires `allow-read` and `allow-write` permissions. */
  export function symlink(
    oldpath: string,
    newpath: string,
    type?: string
  ): Promise<void>;

  /** **UNSTABLE** */
  export type DirKind =
    | "home"
    | "cache"
    | "config"
    | "executable"
    | "data"
    | "data_local"
    | "audio"
    | "desktop"
    | "document"
    | "download"
    | "font"
    | "picture"
    | "public"
    | "template"
    | "tmp"
    | "video";

  /**
   * **UNSTABLE**: Currently under evaluation to decide if method name `dir` and
   * parameter type alias name `DirKind` should be renamed.
   *
   * Returns the user and platform specific directories.
   *
   *       const homeDirectory = Deno.dir("home");
   *
   * Requires `allow-env` permission.
   *
   * Returns `null` if there is no applicable directory or if any other error
   * occurs.
   *
   * Argument values: `"home"`, `"cache"`, `"config"`, `"executable"`, `"data"`,
   * `"data_local"`, `"audio"`, `"desktop"`, `"document"`, `"download"`,
   * `"font"`, `"picture"`, `"public"`, `"template"`, `"tmp"`, `"video"`
   *
   * `"home"`
   *
   * |Platform | Value                                    | Example                |
   * | ------- | -----------------------------------------| -----------------------|
   * | Linux   | `$HOME`                                  | /home/alice            |
   * | macOS   | `$HOME`                                  | /Users/alice           |
   * | Windows | `{FOLDERID_Profile}`                     | C:\Users\Alice         |
   *
   * `"cache"`
   *
   * |Platform | Value                               | Example                      |
   * | ------- | ----------------------------------- | ---------------------------- |
   * | Linux   | `$XDG_CACHE_HOME` or `$HOME`/.cache | /home/alice/.cache           |
   * | macOS   | `$HOME`/Library/Caches              | /Users/Alice/Library/Caches  |
   * | Windows | `{FOLDERID_LocalAppData}`           | C:\Users\Alice\AppData\Local |
   *
   * `"config"`
   *
   * |Platform | Value                                 | Example                          |
   * | ------- | ------------------------------------- | -------------------------------- |
   * | Linux   | `$XDG_CONFIG_HOME` or `$HOME`/.config | /home/alice/.config              |
   * | macOS   | `$HOME`/Library/Preferences           | /Users/Alice/Library/Preferences |
   * | Windows | `{FOLDERID_RoamingAppData}`           | C:\Users\Alice\AppData\Roaming   |
   *
   * `"executable"`
   *
   * |Platform | Value                                                           | Example                |
   * | ------- | --------------------------------------------------------------- | -----------------------|
   * | Linux   | `XDG_BIN_HOME` or `$XDG_DATA_HOME`/../bin or `$HOME`/.local/bin | /home/alice/.local/bin |
   * | macOS   | -                                                               | -                      |
   * | Windows | -                                                               | -                      |
   *
   * `"data"`
   *
   * |Platform | Value                                    | Example                                  |
   * | ------- | ---------------------------------------- | ---------------------------------------- |
   * | Linux   | `$XDG_DATA_HOME` or `$HOME`/.local/share | /home/alice/.local/share                 |
   * | macOS   | `$HOME`/Library/Application Support      | /Users/Alice/Library/Application Support |
   * | Windows | `{FOLDERID_RoamingAppData}`              | C:\Users\Alice\AppData\Roaming           |
   *
   * `"data_local"`
   *
   * |Platform | Value                                    | Example                                  |
   * | ------- | ---------------------------------------- | ---------------------------------------- |
   * | Linux   | `$XDG_DATA_HOME` or `$HOME`/.local/share | /home/alice/.local/share                 |
   * | macOS   | `$HOME`/Library/Application Support      | /Users/Alice/Library/Application Support |
   * | Windows | `{FOLDERID_LocalAppData}`                | C:\Users\Alice\AppData\Local             |
   *
   * `"audio"`
   *
   * |Platform | Value              | Example              |
   * | ------- | ------------------ | -------------------- |
   * | Linux   | `XDG_MUSIC_DIR`    | /home/alice/Music    |
   * | macOS   | `$HOME`/Music      | /Users/Alice/Music   |
   * | Windows | `{FOLDERID_Music}` | C:\Users\Alice\Music |
   *
   * `"desktop"`
   *
   * |Platform | Value                | Example                |
   * | ------- | -------------------- | ---------------------- |
   * | Linux   | `XDG_DESKTOP_DIR`    | /home/alice/Desktop    |
   * | macOS   | `$HOME`/Desktop      | /Users/Alice/Desktop   |
   * | Windows | `{FOLDERID_Desktop}` | C:\Users\Alice\Desktop |
   *
   * `"document"`
   *
   * |Platform | Value                  | Example                  |
   * | ------- | ---------------------- | ------------------------ |
   * | Linux   | `XDG_DOCUMENTS_DIR`    | /home/alice/Documents    |
   * | macOS   | `$HOME`/Documents      | /Users/Alice/Documents   |
   * | Windows | `{FOLDERID_Documents}` | C:\Users\Alice\Documents |
   *
   * `"download"`
   *
   * |Platform | Value                  | Example                  |
   * | ------- | ---------------------- | ------------------------ |
   * | Linux   | `XDG_DOWNLOAD_DIR`     | /home/alice/Downloads    |
   * | macOS   | `$HOME`/Downloads      | /Users/Alice/Downloads   |
   * | Windows | `{FOLDERID_Downloads}` | C:\Users\Alice\Downloads |
   *
   * `"font"`
   *
   * |Platform | Value                                                | Example                        |
   * | ------- | ---------------------------------------------------- | ------------------------------ |
   * | Linux   | `$XDG_DATA_HOME`/fonts or `$HOME`/.local/share/fonts | /home/alice/.local/share/fonts |
   * | macOS   | `$HOME/Library/Fonts`                                | /Users/Alice/Library/Fonts     |
   * | Windows | –                                                    | –                              |
   *
   * `"picture"`
   *
   * |Platform | Value                 | Example                 |
   * | ------- | --------------------- | ----------------------- |
   * | Linux   | `XDG_PICTURES_DIR`    | /home/alice/Pictures    |
   * | macOS   | `$HOME`/Pictures      | /Users/Alice/Pictures   |
   * | Windows | `{FOLDERID_Pictures}` | C:\Users\Alice\Pictures |
   *
   * `"public"`
   *
   * |Platform | Value                 | Example             |
   * | ------- | --------------------- | ------------------- |
   * | Linux   | `XDG_PUBLICSHARE_DIR` | /home/alice/Public  |
   * | macOS   | `$HOME`/Public        | /Users/Alice/Public |
   * | Windows | `{FOLDERID_Public}`   | C:\Users\Public     |
   *
   * `"template"`
   *
   * |Platform | Value                  | Example                                                    |
   * | ------- | ---------------------- | ---------------------------------------------------------- |
   * | Linux   | `XDG_TEMPLATES_DIR`    | /home/alice/Templates                                      |
   * | macOS   | –                      | –                                                          |
   * | Windows | `{FOLDERID_Templates}` | C:\Users\Alice\AppData\Roaming\Microsoft\Windows\Templates |
   *
   * `"tmp"`
   *
   * |Platform | Value                  | Example                                                    |
   * | ------- | ---------------------- | ---------------------------------------------------------- |
   * | Linux   | `TMPDIR`               | /tmp                                                       |
   * | macOS   | `TMPDIR`               | /tmp                                                       |
   * | Windows | `{TMP}`                | C:\Users\Alice\AppData\Local\Temp                          |
   *
   * `"video"`
   *
   * |Platform | Value               | Example               |
   * | ------- | ------------------- | --------------------- |
   * | Linux   | `XDG_VIDEOS_DIR`    | /home/alice/Videos    |
   * | macOS   | `$HOME`/Movies      | /Users/Alice/Movies   |
   * | Windows | `{FOLDERID_Videos}` | C:\Users\Alice\Videos |
   *
   */
  export function dir(kind: DirKind): string | null;

  /** Returns an array containing the 1, 5, and 15 minute load averages. The
   * load average is a measure of CPU and IO utilization of the last one, five,
   * and 15 minute periods expressed as a fractional number.  Zero means there
   * is no load. On Windows, the three values are always the same and represent
   * the current load, not the 1, 5 and 15 minute load averages.
   *
   *       console.log(Deno.loadavg());  // e.g. [ 0.71, 0.44, 0.44 ]
   *
   * Requires `allow-env` permission.
   *
   * **Unstable**  There are questions around which permission this needs. And
   * maybe should be renamed (loadAverage?)
   */
  export function loadavg(): number[];

  /** Returns the release version of the Operating System.
   *
   *       console.log(Deno.osRelease());
   *
   * Requires `allow-env` permission.
   *
   * **Unstable** new API maybe move to Deno.build or Deno.versions? Depends on
   * sys-info, which we don't necessarally want to depend on.
   */
  export function osRelease(): string;

  /** **UNSTABLE**: new API, yet to be vetted.
   *
   * Open and initalize a plugin.
   *
   *        const rid = Deno.openPlugin("./path/to/some/plugin.so");
   *        const opId = Deno.core.ops()["some_op"];
   *        const response = Deno.core.dispatch(opId, new Uint8Array([1,2,3,4]));
   *        console.log(`Response from plugin ${response}`);
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
   *       const [diagnostics, result] = Deno.compile("file_with_compile_issues.ts");
   *       console.table(diagnostics);  // Prints raw diagnostic data
   *       console.log(Deno.formatDiagnostics(diagnostics));  // User friendly output of diagnostics
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
     *      Deno.compile(
     *        "./foo.js",
     *        undefined,
     *        {
     *          types: [ "./foo.d.ts", "https://deno.land/x/example/types.d.ts" ]
     *        }
     *      );
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
   *      const results =  await Deno.transpileOnly({
   *        "foo.ts": `const foo: string = "foo";`
   *      });
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
    options?: CompilerOptions
  ): Promise<Record<string, TranspileOnlyResult>>;

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
   *      const [ maybeDiagnostics1, output1 ] = await Deno.compile("foo.ts");
   *
   *      const [ maybeDiagnostics2, output2 ] = await Deno.compile("/foo.ts", {
   *        "/foo.ts": `export * from "./bar.ts";`,
   *        "/bar.ts": `export const bar = "bar";`
   *      });
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
    options?: CompilerOptions
  ): Promise<[DiagnosticItem[] | undefined, Record<string, string>]>;

  /** **UNSTABLE**: new API, yet to be vetted.
   *
   * `bundle()` is part the compiler API.  A full description of this functionality
   * can be found in the [manual](https://deno.land/std/manual.md#denobundle).
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
   *      // equivalent to "deno bundle foo.ts" from the command line
   *      const [ maybeDiagnostics1, output1 ] = await Deno.bundle("foo.ts");
   *
   *      const [ maybeDiagnostics2, output2 ] = await Deno.bundle("/foo.ts", {
   *        "/foo.ts": `export * from "./bar.ts";`,
   *        "/bar.ts": `export const bar = "bar";`
   *      });
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
    options?: CompilerOptions
  ): Promise<[DiagnosticItem[] | undefined, string]>;

  /** UNSTABLE: new API, yet to be vetted.
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
   *       const orig = Deno.applySourceMap({
   *         fileName: "file://my/module.ts",
   *         lineNumber: 5,
   *         columnNumber: 15
   *       });
   *       console.log(`${orig.filename}:${orig.line}:${orig.column}`);
   */
  export function applySourceMap(location: Location): Location;

  /** **UNSTABLE**: new API, yet to be vetted.
   *
   * Represents the stream of signals, implements both `AsyncIterator` and
   * `PromiseLike`. */
  export class SignalStream
    implements AsyncIterableIterator<void>, PromiseLike<void> {
    constructor(signal: typeof Deno.Signal);
    then<T, S>(
      f: (v: void) => T | Promise<T>,
      g?: (v: void) => S | Promise<S>
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
   *      for await (const _ of Deno.signal(Deno.Signal.SIGTERM)) {
   *        console.log("got SIGTERM!");
   *      }
   *
   * You can also use it as a promise. In this case you can only receive the
   * first one.
   *
   *      await Deno.signal(Deno.Signal.SIGTERM);
   *      console.log("SIGTERM received!")
   *
   * If you want to stop receiving the signals, you can use `.dispose()` method
   * of the signal stream object.
   *
   *      const sig = Deno.signal(Deno.Signal.SIGTERM);
   *      setTimeout(() => { sig.dispose(); }, 5000);
   *      for await (const _ of sig) {
   *        console.log("SIGTERM!")
   *      }
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
   *       Deno.setRaw(myTTY.rid, true);
   */
  export function setRaw(rid: number, mode: boolean): void;

  /** **UNSTABLE**: needs investigation into high precision time.
   *
   * Synchronously changes the access (`atime`) and modification (`mtime`) times
   * of a file system object referenced by `path`. Given times are either in
   * seconds (UNIX epoch time) or as `Date` objects.
   *
   *       Deno.utimeSync("myfile.txt", 1556495550, new Date());
   *
   * Requires `allow-write` permission. */
  export function utimeSync(
    path: string,
    atime: number | Date,
    mtime: number | Date
  ): void;

  /** **UNSTABLE**: needs investigation into high precision time.
   *
   * Changes the access (`atime`) and modification (`mtime`) times of a file
   * system object referenced by `path`. Given times are either in seconds
   * (UNIX epoch time) or as `Date` objects.
   *
   *       await Deno.utime("myfile.txt", 1556495550, new Date());
   *
   * Requires `allow-write` permission. */
  export function utime(
    path: string,
    atime: number | Date,
    mtime: number | Date
  ): Promise<void>;

  /** **UNSTABLE**: Maybe remove `ShutdownMode` entirely.
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
   *       const listener = Deno.listen({ port: 80 });
   *       const conn = await listener.accept();
   *       Deno.shutdown(conn.rid, Deno.ShutdownMode.Write);
   */
  export function shutdown(rid: number, how: ShutdownMode): Promise<void>;

  export interface UnixListenOptions {
    /** A Path to the Unix Socket. */
    address: string;
  }

  /** **UNSTABLE**: new API, yet to be vetted.
   *
   * Listen announces on the local transport address.
   *
   *     const listener = Deno.listen({ address: "/foo/bar.sock", transport: "unix" })
   *
   * Requires `allow-read` and `allow-write` permission. */
  export function listen(
    options: UnixListenOptions & { transport: "unix" }
  ): Listener;

  /** **UNSTABLE**: new API, yet to be vetted.
   *
   * Listen announces on the local transport address.
   *
   *     const listener = Deno.listen({ address: "/foo/bar.sock", transport: "unixpacket" })
   *
   * Requires `allow-read` and `allow-write` permission. */
  export function listen(
    options: UnixListenOptions & { transport: "unixpacket" }
  ): DatagramConn;

  export interface UnixConnectOptions {
    transport: "unix";
    address: string;
  }

  /**
   * Connects to the hostname (default is "127.0.0.1") and port on the named
   * transport (default is "tcp"), and resolves to the connection (`Conn`).
   *
   *     const conn1 = await Deno.connect({ port: 80 });
   *     const conn2 = await Deno.connect({ hostname: "192.0.2.1", port: 80 });
   *     const conn3 = await Deno.connect({ hostname: "[2001:db8::1]", port: 80 });
   *     const conn4 = await Deno.connect({ hostname: "golang.org", port: 80, transport: "tcp" });
   *     const conn5 = await Deno.connect({ address: "/foo/bar.sock", transport: "unix" });
   *
   * Requires `allow-net` permission for "tcp" and `allow-read` for unix. */
  export function connect(
    options: ConnectOptions | UnixConnectOptions
  ): Promise<Conn>;
}

// eslint-disable-next-line @typescript-eslint/no-empty-interface
declare interface WindowOrWorkerGlobalScope {
  //
}
