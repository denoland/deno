// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

// Diagnostic provides an abstraction for advice/errors received from a
// compiler, which is strongly influenced by the format of TypeScript
// diagnostics.

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
  message: string;

  messageChain?: DiagnosticMessageChain;

  relatedInformation?: DiagnosticItem[];

  sourceLine?: string;

  lineNumber?: number;

  scriptResourceName?: string;

  startPosition?: number;

  endPosition?: number;

  category: DiagnosticCategory;

  code: number;

  startColumn?: number;

  endColumn?: number;
}

export interface Diagnostic {
  items: DiagnosticItem[];
}
