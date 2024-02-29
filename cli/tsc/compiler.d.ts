// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

// Contains types that can be used to validate and check `99_main_compiler.js`

import * as _ts from "./dts/typescript.d.ts";

declare global {
  namespace ts {
    var libs: string[];
    var libMap: Map<string, string>;
    var base64encode: (host: ts.CompilerHost, input: string) => string;
    var normalizePath: (path: string) => string;

    interface SourceFile {
      version?: string;
      scriptSnapShot?: _ts.IScriptSnapshot;
    }

    interface CompilerHost {
      base64encode?: (data: any) => string;
    }

    interface Performance {
      enable(): void;
      getDuration(value: string): number;
    }

    var performance: Performance;

    function setLocalizedDiagnosticMessages(
      messages: Record<string, string>,
    ): void;
  }

  namespace ts {
    // @ts-ignore allow using an export = here
    export = _ts;
  }

  interface Object {
    // deno-lint-ignore no-explicit-any
    __proto__: any;
  }

  interface DenoCore {
    encode(value: string): Uint8Array;
    // deno-lint-ignore no-explicit-any
    ops: Record<string, (...args: unknown[]) => any>;
    // deno-lint-ignore no-explicit-any
    asyncOps: Record<string, (...args: unknown[]) => any>;
    print(msg: string, stderr: boolean): void;
    registerErrorClass(
      name: string,
      Ctor: typeof Error,
      // deno-lint-ignore no-explicit-any
      ...args: any[]
    ): void;
  }
}
