// Copyright 2018-2026 the Deno authors. MIT license.
// Copyright Node.js contributors. All rights reserved. MIT License.

// deno-lint-ignore-file ban-types
(function () {
const { core, primordials } = __bootstrap;
const { AssertionError } = core.loadExtScript(
  "ext:deno_node/internal/assert/assertion_error.js",
);
const { isError } = core.loadExtScript("ext:deno_node/internal/util.mjs");
const {
  ERR_AMBIGUOUS_ARGUMENT,
  ERR_INVALID_ARG_TYPE,
  isErrorStackTraceLimitWritable,
} = core.loadExtScript(
  "ext:deno_node/internal/errors.ts",
);
const { format } = core.loadExtScript(
  "ext:deno_node/internal/util/inspect.mjs",
);
const {
  getErrorSourceExpression,
} = core.loadExtScript(
  "ext:deno_node/internal/errors/error_source.ts",
);

const {
  ArrayPrototypeSlice,
  Error,
  ErrorCaptureStackTrace,
  ErrorPrototypeToString,
  SafeArrayIterator,
  SafeRegExp,
  StringPrototypeCharCodeAt,
  StringPrototypeReplace,
} = primordials;

// Escape control characters but not \n and \t to keep the line breaks and
// indentation intact.
// deno-lint-ignore no-control-regex
const escapeSequencesRegExp = new SafeRegExp(/[\x00-\x08\x0b\x0c\x0e-\x1f]/g);
const meta = [
  "\\u0000",
  "\\u0001",
  "\\u0002",
  "\\u0003",
  "\\u0004",
  "\\u0005",
  "\\u0006",
  "\\u0007",
  "\\b",
  "",
  "",
  "\\u000b",
  "\\f",
  "",
  "\\u000e",
  "\\u000f",
  "\\u0010",
  "\\u0011",
  "\\u0012",
  "\\u0013",
  "\\u0014",
  "\\u0015",
  "\\u0016",
  "\\u0017",
  "\\u0018",
  "\\u0019",
  "\\u001a",
  "\\u001b",
  "\\u001c",
  "\\u001d",
  "\\u001e",
  "\\u001f",
];

const escapeFn = (str: string) => meta[StringPrototypeCharCodeAt(str, 0)];

function getErrMessage(fn: Function) {
  // deno-lint-ignore deno-internal/prefer-primordials
  const tmpLimit = Error.stackTraceLimit;
  const errorStackTraceLimitIsWritable = isErrorStackTraceLimitWritable();
  // Make sure the limit is set to 1. Otherwise it could fail (<= 0) or it
  // does too much work.
  // deno-lint-ignore deno-internal/prefer-primordials
  if (errorStackTraceLimitIsWritable) Error.stackTraceLimit = 1;
  // We only need the stack trace. To minimize the overhead use an object
  // instead of an error.
  const err = {};
  ErrorCaptureStackTrace(err, fn);
  // deno-lint-ignore deno-internal/prefer-primordials
  if (errorStackTraceLimitIsWritable) Error.stackTraceLimit = tmpLimit;

  let source = getErrorSourceExpression(err as Error);
  if (source) {
    source = StringPrototypeReplace(source, escapeSequencesRegExp, escapeFn);
    return `The expression evaluated to a falsy value:\n\n  ${source}\n`;
  }
}

type MessageFactory = (actual: unknown, expected: unknown) => unknown;

/**
 * Raw message input is always passed internally as a tuple array;
 *  - `[]`                    : use the default message
 *  - `[string]`              : use as is
 *  - `[string, ...unknown[]]`: print like substitutions
 *  - `[Error]`               : thrown as is
 *  - `[MessageFactory]`      : called with `(actual, expected)`
 */

type MessageTuple =
  | []
  | [string, ...unknown[]]
  | [Error]
  | [MessageFactory];

interface InnerFailOptions {
  actual: unknown;
  expected: unknown;
  message: MessageTuple;
  operator: string;
  stackStartFn: Function;
  diff?: "simple" | "full";
  generatedMessage?: boolean;
}

function innerFail(obj: InnerFailOptions): never {
  const { message } = obj;
  let resolved: string | undefined;

  if (message.length === 0) {
    resolved = undefined;
  } else if (typeof message[0] === "string") {
    resolved = message.length > 1
      ? format(...new SafeArrayIterator(message))
      : message[0];
  } else if (isError(message[0])) {
    if (message.length > 1) {
      throw new ERR_AMBIGUOUS_ARGUMENT(
        "message",
        `The error message was passed as error object "${
          ErrorPrototypeToString(message[0])
        }" has trailing arguments that would be ignored.`,
      );
    }
    throw message[0];
  } else if (typeof message[0] === "function") {
    if (message.length > 1) {
      throw new ERR_AMBIGUOUS_ARGUMENT(
        "message",
        `The error message with function "${
          message[0].name || "anonymous"
        }" has trailing arguments that would be ignored.`,
      );
    }
    try {
      const result = message[0](obj.actual, obj.expected);
      resolved = typeof result === "string" ? result : undefined;
    } catch {
      // Ignore and use the default message instead.
      resolved = undefined;
    }
  } else {
    throw new ERR_INVALID_ARG_TYPE(
      "message",
      ["string", "function"],
      message[0],
    );
  }

  const error = new AssertionError({
    actual: obj.actual,
    expected: obj.expected,
    message: resolved,
    operator: obj.operator,
    stackStartFn: obj.stackStartFn,
    diff: obj.diff,
  });
  if (obj.generatedMessage !== undefined) {
    error.generatedMessage = obj.generatedMessage;
  }
  throw error;
}

function innerOk(fn: Function, ...args: unknown[]) {
  if (!args[0]) {
    let generatedMessage = false;
    let messageArgs: MessageTuple;

    if (args.length === 0) {
      generatedMessage = true;
      messageArgs = ["No value argument passed to `assert.ok()`"];
    } else if (args.length === 1 || args[1] == null) {
      generatedMessage = true;
      // The source expression may be unavailable; fall back to the default
      // message instead of passing `undefined` as the message argument.
      const source = getErrMessage(fn);
      messageArgs = source === undefined ? [] : [source];
    } else {
      messageArgs = ArrayPrototypeSlice(args, 1) as MessageTuple;
    }

    innerFail({
      actual: args[0],
      expected: true,
      message: messageArgs,
      operator: "==",
      stackStartFn: fn,
      generatedMessage,
    });
  }
}

return {
  innerFail,
  innerOk,
};
})();
