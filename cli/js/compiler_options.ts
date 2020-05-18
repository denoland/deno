// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

export interface CompilerOptions {
  allowJs?: boolean;

  allowSyntheticDefaultImports?: boolean;

  allowUmdGlobalAccess?: boolean;

  allowUnreachableCode?: boolean;

  allowUnusedLabels?: boolean;

  alwaysStrict?: boolean;

  baseUrl?: string;

  checkJs?: boolean;

  declaration?: boolean;

  declarationDir?: string;

  declarationMap?: boolean;

  downlevelIteration?: boolean;

  emitBOM?: boolean;

  emitDeclarationOnly?: boolean;

  emitDecoratorMetadata?: boolean;

  esModuleInterop?: boolean;

  experimentalDecorators?: boolean;

  inlineSourceMap?: boolean;

  inlineSources?: boolean;

  isolatedModules?: boolean;

  jsx?: "react" | "preserve" | "react-native";

  jsxFactory?: string;

  keyofStringsOnly?: string;

  useDefineForClassFields?: boolean;

  lib?: string[];

  locale?: string;

  mapRoot?: string;

  module?:
    | "none"
    | "commonjs"
    | "amd"
    | "system"
    | "umd"
    | "es6"
    | "es2015"
    | "esnext";

  noEmitHelpers?: boolean;

  noFallthroughCasesInSwitch?: boolean;

  noImplicitAny?: boolean;

  noImplicitReturns?: boolean;

  noImplicitThis?: boolean;

  noImplicitUseStrict?: boolean;

  noResolve?: boolean;

  noStrictGenericChecks?: boolean;

  noUnusedLocals?: boolean;

  noUnusedParameters?: boolean;

  outDir?: string;

  paths?: Record<string, string[]>;

  preserveConstEnums?: boolean;

  removeComments?: boolean;

  resolveJsonModule?: boolean;

  rootDir?: string;

  rootDirs?: string[];

  sourceMap?: boolean;

  sourceRoot?: string;

  strict?: boolean;

  strictBindCallApply?: boolean;

  strictFunctionTypes?: boolean;

  strictPropertyInitialization?: boolean;

  strictNullChecks?: boolean;

  suppressExcessPropertyErrors?: boolean;

  suppressImplicitAnyIndexErrors?: boolean;

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

  types?: string[];
}
