// Copyright 2018-2026 the Deno authors. MIT license.
// Copyright Node.js contributors. All rights reserved. MIT License.

// deno-lint-ignore-file prefer-primordials

import { primordials } from "ext:core/mod.js";
import {
  op_node_get_first_expression,
  op_require_read_file,
} from "ext:core/ops";
import { Module } from "node:module";

// Type definition from https://github.com/sindresorhus/callsites/blob/v4.2.0/index.d.ts
// MIT License, Copyright (c) Sindre Sorhus <sindresorhus@gmail.com> (https://sindresorhus.com)
// Also see https://v8.dev/docs/stack-trace-api
interface CallSite {
  getThis(): unknown | undefined;
  getTypeName(): string | null;
  getFunction(): (...args: unknown[]) => unknown | undefined;
  getFunctionName(): string | null;
  getMethodName(): string | undefined;
  getFileName(): string | null;
  getLineNumber(): number | null;
  getColumnNumber(): number | null;
  getEvalOrigin(): string | undefined;
  isToplevel(): boolean;
  isEval(): boolean;
  isNative(): boolean;
  isConstructor(): boolean;
  isAsync(): boolean;
  isPromiseAll(): boolean;
  getPromiseIndex(): number | null;
}

const {
  StringPrototypeSlice,
} = primordials;

/**
 * Get the source location of an error using V8's structured call site info.
 *
 * The `error.stack` must not have been accessed. We use Error.prepareStackTrace
 * to get structured call site objects directly from V8.
 *
 * @param error An error object, or an object being invoked with
 *              ErrorCaptureStackTrace
 */
function getErrorSourceLocation(
  error: Error,
): { sourceLine: string; startColumn: number } | undefined {
  // Use Error.prepareStackTrace to get structured call site info from V8.
  // This is the same approach used by node:util.getCallSites.
  const original = Error.prepareStackTrace;
  try {
    Error.prepareStackTrace = (_, stackTraces) => stackTraces;
    const stack = error.stack as unknown as CallSite[];
    if (!stack || !stack.length) {
      return;
    }

    const callSite = stack[0];
    const fileName = callSite.getFileName();
    const lineNumber = callSite.getLineNumber();
    let startColumn = callSite.getColumnNumber();
    if (!fileName || !lineNumber) {
      return;
    }

    let sourceLine;
    try {
      const content: string = op_require_read_file(fileName);
      const lines = content.split("\n");
      if (lineNumber > lines.length) {
        return;
      }

      // lineNumber is 1-based
      sourceLine = lines[lineNumber - 1];
    } catch {
      // Ignore errors
      return;
    }

    if (sourceLine === undefined) {
      return;
    }

    // startColumn from V8 is 1-based, convert to 0-based
    startColumn = typeof startColumn === "number" ? startColumn - 1 : 0;

    // On commonjs modules, the code is wrapped in a function,
    // which offsets the first line of the module.
    // TODO(Tango992): This is a workaround, we should find a better way to get the correct source location.
    if (lineNumber === 1 && startColumn > sourceLine.length) {
      startColumn -= Module.wrapper[0].length;
    }

    return { sourceLine, startColumn };
  } finally {
    Error.prepareStackTrace = original;
  }
}

/**
 * Get the source expression of an error. If source map is enabled, resolve
 * the source location based on the source map.
 *
 * The `error.stack` must not have been accessed, or the source location may
 * be incorrect. The resolution is based on the structured error stack data.
 * @param error An error object, or an object being invoked with
 *              ErrorCaptureStackTrace
 */
function getErrorSourceExpression(error: Error): string | undefined {
  const loc = getErrorSourceLocation(error);
  if (typeof loc === "undefined") {
    return;
  }
  const { sourceLine, startColumn } = loc;
  return getFirstExpression(sourceLine, startColumn);
}

/**
 * Get the first expression in a code string at the startColumn.
 * Delegates to the Rust op which uses deno_ast (SWC) for tokenization.
 * @param code source code line
 * @param startColumn which column the error is constructed
 */
function getFirstExpression(
  code: string,
  startColumn: number,
): string | undefined {
  try {
    return op_node_get_first_expression(code, startColumn);
  } catch {
    // If tokenization fails (e.g., incomplete/invalid JS), return the rest
    // of the line from startColumn
    return StringPrototypeSlice(code, startColumn);
  }
}

export { getErrorSourceExpression, getErrorSourceLocation };
