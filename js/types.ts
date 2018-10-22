// Copyright 2018 the Deno authors. All rights reserved. MIT license.
export type TypedArray = Uint8Array | Float32Array | Int32Array;

// tslint:disable:max-line-length
// Following definitions adapted from:
//   https://github.com/DefinitelyTyped/DefinitelyTyped/blob/master/types/node/index.d.ts
// Type definitions for Node.js 10.3.x
// Definitions by: Microsoft TypeScript <http://typescriptlang.org>
//                 DefinitelyTyped <https://github.com/DefinitelyTyped/DefinitelyTyped>
//                 Parambir Singh <https://github.com/parambirs>
//                 Christian Vaagland Tellnes <https://github.com/tellnes>
//                 Wilco Bakker <https://github.com/WilcoBakker>
//                 Nicolas Voigt <https://github.com/octo-sniffle>
//                 Chigozirim C. <https://github.com/smac89>
//                 Flarna <https://github.com/Flarna>
//                 Mariusz Wiktorczyk <https://github.com/mwiktorczyk>
//                 wwwy3y3 <https://github.com/wwwy3y3>
//                 Deividas Bakanas <https://github.com/DeividasBakanas>
//                 Kelvin Jin <https://github.com/kjin>
//                 Alvis HT Tang <https://github.com/alvis>
//                 Sebastian Silbermann <https://github.com/eps1lon>
//                 Hannes Magnusson <https://github.com/Hannes-Magnusson-CK>
//                 Alberto Schiabel <https://github.com/jkomyno>
//                 Klaus Meinhardt <https://github.com/ajafff>
//                 Huw <https://github.com/hoo29>
//                 Nicolas Even <https://github.com/n-e>
//                 Bruno Scheufler <https://github.com/brunoscheufler>
//                 Mohsen Azimi <https://github.com/mohsen1>
//                 Hoàng Văn Khải <https://github.com/KSXGitHub>
//                 Alexander T. <https://github.com/a-tarasyuk>
//                 Lishude <https://github.com/islishude>
//                 Andrew Makarov <https://github.com/r3nya>
// tslint:enable:max-line-length

export interface CallSite {
  /** Value of `this` */
  // tslint:disable-next-line:no-any
  getThis(): any;

  /** Type of `this` as a string.
   *
   * This is the name of the function stored in the constructor field of
   * `this`, if available.  Otherwise the object's `[[Class]]` internal
   * property.
   */
  getTypeName(): string | null;

  /** Current function. */
  getFunction(): Function | undefined;

  /** Name of the current function, typically its name property.
   *
   * If a name property is not available an attempt will be made to try
   * to infer a name from the function's context.
   */
  getFunctionName(): string | null;

  /** Name of the property (of `this` or one of its prototypes) that holds
   * the current function.
   */
  getMethodName(): string | null;

  /** Name of the script (if this function was defined in a script). */
  getFileName(): string | null;

  /** Get the script name or source URL for the source map. */
  getScriptNameOrSourceURL(): string;

  /** Current line number (if this function was defined in a script). */
  getLineNumber(): number | null;

  /** Current column number (if this function was defined in a script). */
  getColumnNumber(): number | null;

  /** A call site object representing the location where eval was called (if
   * this function was created using a call to `eval`)
   */
  getEvalOrigin(): string | undefined;

  /** Is this a top level invocation, that is, is `this` the global object? */
  isToplevel(): boolean;

  /** Does this call take place in code defined by a call to `eval`? */
  isEval(): boolean;

  /** Is this call in native V8 code? */
  isNative(): boolean;

  /** Is this a constructor call? */
  isConstructor(): boolean;
}

export interface StartOfSourceMap {
  file?: string;
  sourceRoot?: string;
}

export interface RawSourceMap extends StartOfSourceMap {
  version: string;
  sources: string[];
  names: string[];
  sourcesContent?: string[];
  mappings: string;
}

declare global {
  // Declare "static" methods in Error
  interface ErrorConstructor {
    /** Create `.stack` property on a target object */
    captureStackTrace(targetObject: object, constructorOpt?: Function): void;

    // tslint:disable:max-line-length
    /**
     * Optional override for formatting stack traces
     *
     * @see https://github.com/v8/v8/wiki/Stack%20Trace%20API#customizing-stack-traces
     */
    // tslint:enable:max-line-length
    // tslint:disable-next-line:no-any
    prepareStackTrace?: (err: Error, stackTraces: CallSite[]) => any;

    stackTraceLimit: number;
  }
}
