// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

// Contains types that can be used to validate and check `99_main_compiler.js`

import * as _ts from "../dts/typescript";

declare global {
  namespace ts {
    var libs: string[];
    var libMap: Map<string, string>;
    var base64encode: (host: ts.CompilerHost, input: string) => string;
    var normalizePath: (path: string) => string;
    interface SourceFile {
      version?: string;
    }

    interface CompilerHost {
      base64encode?: (data: any) => string;
    }

    interface Performance {
      enable(): void;
      getDuration(value: string): number;
    }

    var performance: Performance;
  }

  namespace ts {
    export = _ts;
  }

  interface Object {
    // deno-lint-ignore no-explicit-any
    __proto__: any;
  }

  interface DenoCore {
    encode(value: string): Uint8Array;
    // deno-lint-ignore no-explicit-any
    opSync<T>(name: string, params: T): any;
    // deno-lint-ignore no-explicit-any
    ops: Record<string, (...args: unknown[]) => any>;
    print(msg: string, stderr: boolean): void;
    registerErrorClass(
      name: string,
      Ctor: typeof Error,
      // deno-lint-ignore no-explicit-any
      ...args: any[]
    ): void;
  }

  type LanguageServerRequest =
    | Restart
    | ConfigureRequest
    | FindRenameLocationsRequest
    | GetAssets
    | GetApplicableRefactors
    | GetEditsForRefactor
    | GetCodeFixes
    | GetCombinedCodeFix
    | GetCompletionDetails
    | GetCompletionsRequest
    | GetDefinitionRequest
    | GetDiagnosticsRequest
    | GetDocumentHighlightsRequest
    | GetEncodedSemanticClassifications
    | GetImplementationRequest
    | GetNavigateToItems
    | GetNavigationTree
    | GetOutliningSpans
    | GetQuickInfoRequest
    | GetReferencesRequest
    | GetSignatureHelpItemsRequest
    | GetSmartSelectionRange
    | GetSupportedCodeFixes
    | GetTypeDefinitionRequest
    | PrepareCallHierarchy
    | ProvideCallHierarchyIncomingCalls
    | ProvideCallHierarchyOutgoingCalls;

  interface BaseLanguageServerRequest {
    id: number;
    method: string;
  }

  interface ConfigureRequest extends BaseLanguageServerRequest {
    method: "configure";
    // deno-lint-ignore no-explicit-any
    compilerOptions: Record<string, any>;
  }

  interface FindRenameLocationsRequest extends BaseLanguageServerRequest {
    method: "findRenameLocations";
    specifier: string;
    position: number;
    findInStrings: boolean;
    findInComments: boolean;
    providePrefixAndSuffixTextForRename: boolean;
  }

  interface GetAssets extends BaseLanguageServerRequest {
    method: "getAssets";
  }

  interface GetApplicableRefactors extends BaseLanguageServerRequest {
    method: "getApplicableRefactors";
    specifier: string;
    range: ts.TextRange;
    kind: string;
  }

  interface GetEditsForRefactor extends BaseLanguageServerRequest {
    method: "getEditsForRefactor";
    specifier: string;
    range: ts.TextRange;
    refactorName: string;
    actionName: string;
  }

  interface GetCodeFixes extends BaseLanguageServerRequest {
    method: "getCodeFixes";
    specifier: string;
    startPosition: number;
    endPosition: number;
    errorCodes: string[];
  }

  interface GetCombinedCodeFix extends BaseLanguageServerRequest {
    method: "getCombinedCodeFix";
    specifier: string;
    // deno-lint-ignore ban-types
    fixId: {};
  }

  interface GetCompletionDetails extends BaseLanguageServerRequest {
    method: "getCompletionDetails";
    args: {
      specifier: string;
      position: number;
      name: string;
      source?: string;
      preferences?: ts.UserPreferences;
      data?: ts.CompletionEntryData;
    };
  }

  interface GetCompletionsRequest extends BaseLanguageServerRequest {
    method: "getCompletions";
    specifier: string;
    position: number;
    preferences: ts.GetCompletionsAtPositionOptions;
  }

  interface GetDiagnosticsRequest extends BaseLanguageServerRequest {
    method: "getDiagnostics";
    specifiers: string[];
  }

  interface GetDefinitionRequest extends BaseLanguageServerRequest {
    method: "getDefinition";
    specifier: string;
    position: number;
  }

  interface GetDocumentHighlightsRequest extends BaseLanguageServerRequest {
    method: "getDocumentHighlights";
    specifier: string;
    position: number;
    filesToSearch: string[];
  }

  interface GetEncodedSemanticClassifications
    extends BaseLanguageServerRequest {
    method: "getEncodedSemanticClassifications";
    specifier: string;
    span: ts.TextSpan;
  }

  interface GetImplementationRequest extends BaseLanguageServerRequest {
    method: "getImplementation";
    specifier: string;
    position: number;
  }

  interface GetNavigateToItems extends BaseLanguageServerRequest {
    method: "getNavigateToItems";
    search: string;
    maxResultCount?: number;
    fileName?: string;
  }

  interface GetNavigationTree extends BaseLanguageServerRequest {
    method: "getNavigationTree";
    specifier: string;
  }

  interface GetOutliningSpans extends BaseLanguageServerRequest {
    method: "getOutliningSpans";
    specifier: string;
  }

  interface GetQuickInfoRequest extends BaseLanguageServerRequest {
    method: "getQuickInfo";
    specifier: string;
    position: number;
  }

  interface GetReferencesRequest extends BaseLanguageServerRequest {
    method: "getReferences";
    specifier: string;
    position: number;
  }

  interface GetSignatureHelpItemsRequest extends BaseLanguageServerRequest {
    method: "getSignatureHelpItems";
    specifier: string;
    position: number;
    options: ts.SignatureHelpItemsOptions;
  }

  interface GetSmartSelectionRange extends BaseLanguageServerRequest {
    method: "getSmartSelectionRange";
    specifier: string;
    position: number;
  }

  interface GetSupportedCodeFixes extends BaseLanguageServerRequest {
    method: "getSupportedCodeFixes";
  }

  interface GetTypeDefinitionRequest extends BaseLanguageServerRequest {
    method: "getTypeDefinition";
    specifier: string;
    position: number;
  }

  interface PrepareCallHierarchy extends BaseLanguageServerRequest {
    method: "prepareCallHierarchy";
    specifier: string;
    position: number;
  }

  interface ProvideCallHierarchyIncomingCalls
    extends BaseLanguageServerRequest {
    method: "provideCallHierarchyIncomingCalls";
    specifier: string;
    position: number;
  }

  interface ProvideCallHierarchyOutgoingCalls
    extends BaseLanguageServerRequest {
    method: "provideCallHierarchyOutgoingCalls";
    specifier: string;
    position: number;
  }

  interface Restart extends BaseLanguageServerRequest {
    method: "restart";
  }
}
