// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

// Contains types that can be used to validate and check `99_main_compiler.js`

import * as _ts from "../dts/typescript";

declare global {
  // deno-lint-ignore no-namespace
  namespace ts {
    var libs: string[];
    var libMap: Map<string, string>;

    interface SourceFile {
      version?: string;
    }

    interface Performance {
      enable(): void;
      getDuration(value: string): number;
    }

    var performance: Performance;
  }

  // deno-lint-ignore no-namespace
  namespace ts {
    export = _ts;
  }

  interface Object {
    // deno-lint-ignore no-explicit-any
    __proto__: any;
  }

  interface DenoCore {
    // deno-lint-ignore no-explicit-any
    jsonOpSync<T>(name: string, params: T): any;
    ops(): void;
    print(msg: string, code?: number): void;
    registerErrorClass(name: string, Ctor: typeof Error): void;
  }

  type LanguageServerRequest =
    | ConfigureRequest
    | GetAsset
    | GetDiagnosticsRequest
    | GetQuickInfoRequest
    | GetDocumentHighlightsRequest
    | GetReferencesRequest
    | GetDefinitionRequest
    | GetCompletionsRequest
    | GetImplementationRequest
    | FindRenameLocationsRequest;

  interface BaseLanguageServerRequest {
    id: number;
    method: string;
  }

  interface ConfigureRequest extends BaseLanguageServerRequest {
    method: "configure";
    // deno-lint-ignore no-explicit-any
    compilerOptions: Record<string, any>;
  }

  interface GetAsset extends BaseLanguageServerRequest {
    method: "getAsset";
    specifier: string;
  }

  interface GetDiagnosticsRequest extends BaseLanguageServerRequest {
    method: "getDiagnostics";
    specifier: string;
  }

  interface GetQuickInfoRequest extends BaseLanguageServerRequest {
    method: "getQuickInfo";
    specifier: string;
    position: number;
  }

  interface GetDocumentHighlightsRequest extends BaseLanguageServerRequest {
    method: "getDocumentHighlights";
    specifier: string;
    position: number;
    filesToSearch: string[];
  }

  interface GetReferencesRequest extends BaseLanguageServerRequest {
    method: "getReferences";
    specifier: string;
    position: number;
  }

  interface GetDefinitionRequest extends BaseLanguageServerRequest {
    method: "getDefinition";
    specifier: string;
    position: number;
  }

  interface GetCompletionsRequest extends BaseLanguageServerRequest {
    method: "getCompletions";
    specifier: string;
    position: number;
    preferences: ts.UserPreferences;
  }

  interface GetImplementationRequest extends BaseLanguageServerRequest {
    method: "getImplementation";
    specifier: string;
    position: number;
  }

  interface FindRenameLocationsRequest extends BaseLanguageServerRequest {
    method: "findRenameLocations";
    specifier: string;
    position: number;
    findInStrings: boolean;
    findInComments: boolean;
    providePrefixAndSuffixTextForRename: boolean;
  }
}
