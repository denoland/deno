// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

// Diagnostic provides an abstraction for advice/errors received from a
// compiler, which is strongly influenced by the format of TypeScript
// diagnostics.

/** The log category for a diagnostic message */
export enum DiagnosticCategory {
  Log = 0,
  Debug = 1,
  Info = 2,
  Error = 3,
  Warning = 4,
  Suggestion = 5
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

  /** Information related to the diagnostic.  This is present when there is a
   * suggestion or other additional diagnostic information */
  relatedInformation?: DiagnosticItem[];

  /** The text of the source line related to the diagnostic */
  sourceLine?: string;

  /** The line number that is related to the diagnostic */
  lineNumber?: number;

  /** The name of the script resource related to the diagnostic */
  scriptResourceName?: string;

  /** The start position related to the diagnostic */
  startPosition?: number;

  /** The end position related to the diagnostic */
  endPosition?: number;

  /** The category of the diagnostic */
  category: DiagnosticCategory;

  /** A number identifier */
  code: number;

  /** The the start column of the sourceLine related to the diagnostic */
  startColumn?: number;

  /** The end column of the sourceLine related to the diagnostic */
  endColumn?: number;
}

export interface Diagnostic {
  /** An array of diagnostic items. */
  items: DiagnosticItem[];
}
