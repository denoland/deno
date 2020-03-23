// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

// These utilities are used by compiler.ts to format TypeScript diagnostics
// into Deno Diagnostics.

import {
  Diagnostic,
  DiagnosticCategory,
  DiagnosticMessageChain,
  DiagnosticItem,
} from "./diagnostics.ts";

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
        `Unexpected DiagnosticCategory: "${category}"/"${ts.DiagnosticCategory[category]}"`
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
    character: startColumn,
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
    endColumn,
  };
}

function fromDiagnosticMessageChain(
  messageChain: ts.DiagnosticMessageChain[] | undefined
): DiagnosticMessageChain[] | undefined {
  if (!messageChain) {
    return undefined;
  }

  return messageChain.map(({ messageText: message, code, category, next }) => {
    return {
      message,
      code,
      category: fromDiagnosticCategory(category),
      next: fromDiagnosticMessageChain(next),
    };
  });
}

function parseDiagnostic(
  item: ts.Diagnostic | ts.DiagnosticRelatedInformation
): DiagnosticItem {
  const {
    messageText,
    category: sourceCategory,
    code,
    file,
    start: startPosition,
    length,
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
    messageChain = fromDiagnosticMessageChain([messageText])![0];
  }

  const base = {
    message,
    messageChain,
    code,
    category,
    startPosition,
    endPosition,
  };

  return sourceInfo ? { ...base, ...sourceInfo } : base;
}

function parseRelatedInformation(
  relatedInformation: readonly ts.DiagnosticRelatedInformation[]
): DiagnosticItem[] {
  const result: DiagnosticItem[] = [];
  for (const item of relatedInformation) {
    result.push(parseDiagnostic(item));
  }
  return result;
}

export function fromTypeScriptDiagnostic(
  diagnostics: readonly ts.Diagnostic[]
): Diagnostic {
  const items: DiagnosticItem[] = [];
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
