// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.

// Diagnostic provides an abstraction for advice/errors received from a
// compiler, which is strongly influenced by the format of TypeScript
// diagnostics.

import * as ts from "typescript";

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
  next?: DiagnosticMessageChain;
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

interface SourceInformation {
  sourceLine: string;
  lineNumber: number;
  scriptResourceName: string;
  startColumn: number;
  endColumn: number;
}

function fromDiagnosticCategory(
  category: ts.DiagnosticCategory
): DiagnosticCategory {
  switch (category) {
    case ts.DiagnosticCategory.Error:
      return DiagnosticCategory.Error;
    case ts.DiagnosticCategory.Message:
      return DiagnosticCategory.Info;
    case ts.DiagnosticCategory.Suggestion:
      return DiagnosticCategory.Suggestion;
    case ts.DiagnosticCategory.Warning:
      return DiagnosticCategory.Warning;
    default:
      throw new Error(
        `Unexpected DiagnosticCategory: "${category}"/"${
          ts.DiagnosticCategory[category]
        }"`
      );
  }
}

function getSourceInformation(
  sourceFile: ts.SourceFile,
  start: number,
  length: number
): SourceInformation {
  const scriptResourceName = sourceFile.fileName;
  const {
    line: lineNumber,
    character: startColumn
  } = sourceFile.getLineAndCharacterOfPosition(start);
  const endPosition = sourceFile.getLineAndCharacterOfPosition(start + length);
  const endColumn =
    lineNumber === endPosition.line ? endPosition.character : startColumn;
  const lastLineInFile = sourceFile.getLineAndCharacterOfPosition(
    sourceFile.text.length
  ).line;
  const lineStart = sourceFile.getPositionOfLineAndCharacter(lineNumber, 0);
  const lineEnd =
    lineNumber < lastLineInFile
      ? sourceFile.getPositionOfLineAndCharacter(lineNumber + 1, 0)
      : sourceFile.text.length;
  const sourceLine = sourceFile.text
    .slice(lineStart, lineEnd)
    .replace(/\s+$/g, "")
    .replace("\t", " ");
  return {
    sourceLine,
    lineNumber,
    scriptResourceName,
    startColumn,
    endColumn
  };
}

/** Converts a TypeScript diagnostic message chain to a Deno one. */
function fromDiagnosticMessageChain(
  messageChain: ts.DiagnosticMessageChain | undefined
): DiagnosticMessageChain | undefined {
  if (!messageChain) {
    return undefined;
  }

  const { messageText: message, code, category, next } = messageChain;
  return {
    message,
    code,
    category: fromDiagnosticCategory(category),
    next: fromDiagnosticMessageChain(next)
  };
}

/** Parse out information from a TypeScript diagnostic structure. */
function parseDiagnostic(
  item: ts.Diagnostic | ts.DiagnosticRelatedInformation
): DiagnosticItem {
  const {
    messageText,
    category: sourceCategory,
    code,
    file,
    start: startPosition,
    length
  } = item;
  const sourceInfo =
    file && startPosition && length
      ? getSourceInformation(file, startPosition, length)
      : undefined;
  const endPosition =
    startPosition && length ? startPosition + length : undefined;
  const category = fromDiagnosticCategory(sourceCategory);

  let message: string;
  let messageChain: DiagnosticMessageChain | undefined;
  if (typeof messageText === "string") {
    message = messageText;
  } else {
    message = messageText.messageText;
    messageChain = fromDiagnosticMessageChain(messageText);
  }

  const base = {
    message,
    messageChain,
    code,
    category,
    startPosition,
    endPosition
  };

  return sourceInfo ? { ...base, ...sourceInfo } : base;
}

/** Convert a diagnostic related information array into a Deno diagnostic
 * array. */
function parseRelatedInformation(
  relatedInformation: readonly ts.DiagnosticRelatedInformation[]
): DiagnosticItem[] {
  const result: DiagnosticItem[] = [];
  for (const item of relatedInformation) {
    result.push(parseDiagnostic(item));
  }
  return result;
}

/** Convert TypeScript diagnostics to Deno diagnostics. */
export function fromTypeScriptDiagnostic(
  diagnostics: readonly ts.Diagnostic[]
): Diagnostic {
  let items: DiagnosticItem[] = [];
  for (const sourceDiagnostic of diagnostics) {
    const item: DiagnosticItem = parseDiagnostic(sourceDiagnostic);
    if (sourceDiagnostic.relatedInformation) {
      item.relatedInformation = parseRelatedInformation(
        sourceDiagnostic.relatedInformation
      );
    }
    items.push(item);
  }
  return { items };
}
