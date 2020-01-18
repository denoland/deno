// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.

// This file contains the runtime APIs which will dispatch work to the internal
// compiler within Deno.

import { Diagnostic } from "./diagnostics.ts";
import * as dispatch from "./dispatch.ts";
import { sendAsync } from "./dispatch_json.ts";
import * as util from "./util.ts";

/** A specific subset TypeScript compiler options that can be supported by
 * the Deno TypeScript compiler. */
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
   * destructuring when targeting ES5 or ES3.  Defaults to `false`. */
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

  /** The locale to use to show error messages. */
  locale?: string;

  /** Specifies the location where debugger should locate map files instead of
   * generated locations. Use this flag if the `.map` files will be located at
   * run-time in a different location than the `.js` files. The location
   * specified will be embedded in the source map to direct the debugger where
   * the map files will be located. Defaults to `undefined`. */
  mapRoot?: string;

  /** Specify the module format for the emitted code.  Defaults to
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
   * `Deno.compile` and only changes the emitted file names.  Defaults to
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

  /** List of names of type definitions to include. Defaults to `undefined`. */
  types?: string[];
}

/** Internal function to just validate that the specifier looks relative, that
 * it starts with `./`. */
function checkRelative(specifier: string): string {
  return specifier.match(/^([\.\/\\]|https?:\/{2}|file:\/{2})/)
    ? specifier
    : `./${specifier}`;
}

/** The results of a transpile only command, where the `source` contains the
 * emitted source, and `map` optionally contains the source map.
 */
export interface TranspileOnlyResult {
  source: string;
  map?: string;
}

/** Takes a set of TypeScript sources and resolves with a map where the key was
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
 *                to transpile.  The filename is only used in the transpile and
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
): Promise<Record<string, TranspileOnlyResult>> {
  util.log("Deno.transpileOnly", { sources: Object.keys(sources), options });
  const payload = {
    sources,
    options: options ? JSON.stringify(options) : undefined
  };
  return sendAsync(dispatch.OP_TRANSPILE, payload).then(result =>
    JSON.parse(result)
  );
}

/** Takes a root module name, any optionally a record set of sources. Resolves
 * with a compiled set of modules.  If just a root name is provided, the modules
 * will be resolved as if the root module had been passed on the command line.
 *
 * If sources are passed, all modules will be resolved out of this object, where
 * the key is the module name and the value is the content.  The extension of
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
 *                 "starting point".  If no `sources` is specified, Deno will
 *                 resolve the module externally as if the `rootName` had been
 *                 specified on the command line.
 * @param sources An optional key/value map of sources to be used when resolving
 *                modules, where the key is the module name, and the value is
 *                the source content.  The extension of the key will determine
 *                the media type of the file when processing.  If supplied,
 *                Deno will not attempt to resolve any modules externally.
 * @param options An optional object of options to send to the compiler. This is
 *                a subset of ts.CompilerOptions which can be supported by Deno.
 */
export function compile(
  rootName: string,
  sources?: Record<string, string>,
  options?: CompilerOptions
): Promise<[Diagnostic | undefined, Record<string, string>]> {
  const payload = {
    rootName: sources ? rootName : checkRelative(rootName),
    sources,
    options: options ? JSON.stringify(options) : undefined,
    bundle: false
  };
  util.log("Deno.compile", {
    rootName: payload.rootName,
    sources: !!sources,
    options
  });
  return sendAsync(dispatch.OP_COMPILE, payload).then(result =>
    JSON.parse(result)
  );
}

/** Takes a root module name, and optionally a record set of sources. Resolves
 * with a single JavaScript string that is like the output of a `deno bundle`
 * command.  If just a root name is provided, the modules will be resolved as if
 * the root module had been passed on the command line.
 *
 * If sources are passed, all modules will be resolved out of this object, where
 * the key is the module name and the value is the content. The extension of the
 * module name will be used to determine the media type of the module.
 *
 *      const [ maybeDiagnostics1, output1 ] = await Deno.bundle("foo.ts");
 *
 *      const [ maybeDiagnostics2, output2 ] = await Deno.bundle("/foo.ts", {
 *        "/foo.ts": `export * from "./bar.ts";`,
 *        "/bar.ts": `export const bar = "bar";`
 *      });
 *
 * @param rootName The root name of the module which will be used as the
 *                 "starting point".  If no `sources` is specified, Deno will
 *                 resolve the module externally as if the `rootName` had been
 *                 specified on the command line.
 * @param sources An optional key/value map of sources to be used when resolving
 *                modules, where the key is the module name, and the value is
 *                the source content.  The extension of the key will determine
 *                the media type of the file when processing.  If supplied,
 *                Deno will not attempt to resolve any modules externally.
 * @param options An optional object of options to send to the compiler. This is
 *                a subset of ts.CompilerOptions which can be supported by Deno.
 */
export function bundle(
  rootName: string,
  sources?: Record<string, string>,
  options?: CompilerOptions
): Promise<[Diagnostic | undefined, string]> {
  const payload = {
    rootName: sources ? rootName : checkRelative(rootName),
    sources,
    options: options ? JSON.stringify(options) : undefined,
    bundle: true
  };
  util.log("Deno.bundle", {
    rootName: payload.rootName,
    sources: !!sources,
    options
  });
  return sendAsync(dispatch.OP_COMPILE, payload).then(result =>
    JSON.parse(result)
  );
}
