// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.

// This mirrors structures found in /core/diagnostics.rs and unifies all types
// of errors and diagnostic messages supported within Deno.

import * as ts from "typescript";

/** The log category for a diagnostic message */
export enum DenoDiagnosticCategory {
  Log = 0,
  Debug = 1,
  Info = 2,
  Error = 3,
  Warning = 4,
  Suggestion = 5
}

/** The source of the diagnostic message */
export enum DenoDiagnosticSources {
  V8 = 0,
  Rust = 1,
  TypeScript = 2,
  Runtime = 3
}

/** A diagnostic frame */
export interface DenoDiagnosticFrame {
  line: number;
  column: number;
  functionName: string;
  scriptName: string;
  isEval: boolean;
  isConstructor: boolean;
  isWasm: boolean;
}

export interface DenoDiagnostic {
  /** A string message summarizing the diagnostic. */
  message: string;

  /** An ordered array of further diagnostics. */
  diagnostics?: DenoDiagnostic[];

  /** Information related to the diagnostic.  This is present when there is a
   * suggestion or other additional diagnostic information */
  relatedInformation?: DenoDiagnostic[];

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
  category: DenoDiagnosticCategory;

  /** A number identifier */
  code?: number;

  /** The origin of the diagnostic */
  source: DenoDiagnosticSources;

  /** The the start column of the sourceLine related to the diagnostic */
  startColumn?: number;

  /** The end column of the sourceLine related to the diagnostic */
  endColumn?: number;

  /** Any frames of a stack trace related to the diagnostic */
  frames?: DenoDiagnosticFrame[];

  /** The next diagnostic in the chain. */
  next?: DenoDiagnostic;
}

interface SourceInformation {
  sourceLine: string;
  lineNumber: number;
  scriptResourceName: string;
  startColumn: number;
  endColumn: number;
}

function toDenoCategory(
  category: ts.DiagnosticCategory
): DenoDiagnosticCategory {
  switch (category) {
    case ts.DiagnosticCategory.Error:
      return DenoDiagnosticCategory.Error;
    case ts.DiagnosticCategory.Message:
      return DenoDiagnosticCategory.Info;
    case ts.DiagnosticCategory.Suggestion:
      return DenoDiagnosticCategory.Suggestion;
    case ts.DiagnosticCategory.Warning:
      return DenoDiagnosticCategory.Warning;
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

/** Parse out information from a TypeScript diagnostic structure. */
function parseDiagnostic(
  item: ts.Diagnostic | ts.DiagnosticRelatedInformation
): DenoDiagnostic {
  const source = DenoDiagnosticSources.TypeScript;
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
  const category = toDenoCategory(sourceCategory);

  let message: string;
  let diagnostics: DenoDiagnostic[] | undefined;
  if (typeof messageText === "string") {
    message = messageText;
  } else {
    message = messageText.messageText;
    let messageChain = messageText;
    diagnostics = [];
    do {
      const { messageText: message, code, category } = messageChain;
      diagnostics.push({
        message,
        code,
        category: toDenoCategory(category),
        source
      });
    } while (messageChain.next && (messageChain = messageChain.next));
  }

  const base = {
    message,
    diagnostics,
    code,
    category,
    source,
    startPosition,
    endPosition
  };

  return sourceInfo ? { ...base, ...sourceInfo } : base;
}

/** Convert a diagnostic related information array into a Deno diagnostic
 * array. */
function parseRelatedInformation(
  relatedInformation: ts.DiagnosticRelatedInformation[]
): DenoDiagnostic[] {
  const result: DenoDiagnostic[] = [];
  for (const item of relatedInformation) {
    result.push(parseDiagnostic(item));
  }
  return result;
}

/** Convert TypeScript diagnostics to DenoDiagnostics. */
export function toDenoDiagnostics(
  diagnostics: ts.Diagnostic[]
): DenoDiagnostic {
  let result: DenoDiagnostic | undefined;
  for (const sourceDiagnostic of diagnostics) {
    const d: DenoDiagnostic = parseDiagnostic(sourceDiagnostic);
    if (sourceDiagnostic.relatedInformation) {
      d.relatedInformation = parseRelatedInformation(
        sourceDiagnostic.relatedInformation
      );
    }
    if (result) {
      result.next = d;
    } else {
      result = d;
    }
  }
  return result!;
}
