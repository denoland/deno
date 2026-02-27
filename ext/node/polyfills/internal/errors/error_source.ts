// Copyright 2018-2026 the Deno authors. MIT license.
// Copyright Node.js contributors. All rights reserved. MIT License.

// deno-lint-ignore-file prefer-primordials

import { primordials } from "ext:core/mod.js";
import {
  op_node_get_error_source_position,
  op_node_get_first_expression,
} from "ext:core/ops";
import Module from "node:module";

type ErrSourcePosition = {
  sourceLine: string;
  lineNumber: number;
  startColumn: number;
};

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
  const pos: ErrSourcePosition | undefined = op_node_get_error_source_position(
    error,
  );
  if (typeof pos === "undefined") {
    return;
  }

  let {
    sourceLine,
    lineNumber,
    startColumn,
  } = pos;

  // On commonjs modules, the code is wrapped in a function,
  // which offsets the first line of the module.
  // TODO(Tango992): This is a workaround, we should find a better way to get the correct source location.
  if (lineNumber === 1 && startColumn > sourceLine.length) {
    startColumn -= Module.wrapper[0].length;
  }

  return { sourceLine, startColumn };
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
function getFirstExpression(code: string, startColumn: number): string {
  // [start, end] of the first expression will be written to resultBuf by the op.
  const resultBuf = new Uint32Array(2);
  op_node_get_first_expression(code, startColumn, resultBuf);
  return StringPrototypeSlice(code, resultBuf[0], resultBuf[1]);
}

export { getErrorSourceExpression, getErrorSourceLocation };
