// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { close } from "./files.ts";
import { exit } from "./os.ts";
import { core } from "./core.ts";
import { formatError } from "./format_error.ts";
import { stringifyArgs } from "./console.ts";
import * as dispatch from "./dispatch.ts";
import { sendSync, sendAsync } from "./dispatch_json.ts";
import { compile } from "./compiler_api.ts";
import diff, { DiffResult, DiffType } from "./diff.ts";

/**
 * REPL logging.
 * In favor of console.log to avoid unwanted indentation
 */
function replLog(...args: unknown[]): void {
  core.print(stringifyArgs(args) + "\n");
}

/**
 * REPL logging for errors.
 * In favor of console.error to avoid unwanted indentation
 */
function replError(...args: unknown[]): void {
  core.print(stringifyArgs(args) + "\n", true);
}

const helpMsg = [
  "_       Get last evaluation result",
  "_error  Get last thrown error",
  "exit    Exit the REPL",
  "help    Print this help message"
].join("\n");

const replCommands = {
  exit: {
    get(): void {
      exit(0);
    }
  },
  help: {
    get(): string {
      return helpMsg;
    }
  }
};

function startRepl(historyFile: string): number {
  return sendSync(dispatch.OP_REPL_START, { historyFile });
}

// @internal
export async function readline(rid: number, prompt: string): Promise<string> {
  return sendAsync(dispatch.OP_REPL_READLINE, { rid, prompt });
}

const recoverableDiagnosticCodes = [
  1003, // "Identifier expected."
  1005, // "')' expected."
  1109, // "Expression expected."
  1126, // "Unexpected end of text."
  1160 // "Unterminated template literal."
];

function isRecoverableDiagnostic(code: number): boolean {
  return recoverableDiagnosticCodes.includes(code);
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
type Value = any;

let lastEvalResult: Value = undefined;
let lastThrownError: Value = undefined;

// Evaluate code.
///
/// Prints return value of thrown error.
function evaluate(code: string): void {
  const [result, errInfo] = core.evalContext(code);
  if (!errInfo) {
    lastEvalResult = result;
    replLog(result);
  } else {
    lastThrownError = errInfo.thrown;
    if (errInfo.isNativeError) {
      const formattedError = formatError(
        core.errorToJSON(errInfo.thrown as Error)
      );
      replError(formattedError);
    } else {
      replError("Thrown:", errInfo.thrown);
    }
  }
}

// @internal
export async function replLoop(): Promise<void> {
  const { console } = globalThis;
  Object.defineProperties(globalThis, replCommands);

  const historyFile = "deno_history.txt";
  const rid = startRepl(historyFile);

  const quitRepl = (exitCode: number): void => {
    // Special handling in case user calls deno.close(3).
    try {
      close(rid); // close signals Drop on REPL and saves history.
    } catch {}
    exit(exitCode);
  };

  // Configure globalThis._ to give the last evaluation result.
  Object.defineProperty(globalThis, "_", {
    configurable: true,
    get: (): Value => lastEvalResult,
    set: (value: Value): Value => {
      Object.defineProperty(globalThis, "_", {
        value: value,
        writable: true,
        enumerable: true,
        configurable: true
      });
      console.log("Last evaluation result is no longer saved to _.");
    }
  });

  // Configure globalThis._error to give the last thrown error.
  Object.defineProperty(globalThis, "_error", {
    configurable: true,
    get: (): Value => lastThrownError,
    set: (value: Value): Value => {
      Object.defineProperty(globalThis, "_error", {
        value: value,
        writable: true,
        enumerable: true,
        configurable: true
      });
      console.log("Last thrown error is no longer saved to _error.");
    }
  });

  const totalCode: string[] = [];
  let previousCompiledCode: string[] = [];

  while (true) {
    let code = "";
    // Top level read
    try {
      code = await readline(rid, "> ");
      if (code.trim() === "") {
        continue;
      }
    } catch (err) {
      if (err.message === "EOF") {
        quitRepl(0);
      } else {
        // If interrupted, don't print error.
        if (err.message !== "Interrupted") {
          // e.g. this happens when we have deno.close(3).
          // We want to display the problem.
          const formattedError = formatError(core.errorToJSON(err));
          replError(formattedError);
        }
        // Quit REPL anyways.
        quitRepl(1);
      }
    }

    while (true) {
      totalCode.push(`${code}\n`);
      const [diagnostics, output] = await compile("<eval>.ts", {
        "<eval>.ts": totalCode.join("\n")
      });

      if (diagnostics) {
        // console.log("diagnostics emitted");

        let isRecoverable = true;

        for (const diagnostic of diagnostics.items) {
          isRecoverable =
            isRecoverable && isRecoverableDiagnostic(diagnostic.code);
          // console.log("diagnostic code: ", diagnostic.code, "recover: ", isRecoverableDiagnostic(diagnostic.code), "message: ", diagnostic.message);

          if (!isRecoverableDiagnostic(diagnostic.code)) {
            const e = new Error(`fatal TS error: ${diagnostic.message}`);
            replError(e);
          }
        }

        if (!isRecoverable) {
          totalCode.pop();
          break;
        } else {
          code += "\n";
          try {
            code += await readline(rid, "  ");
          } catch (err) {
            // If interrupted on continued read,
            // abort this read instead of quitting.
            if (err.message === "Interrupted") {
              break;
            } else if (err.message === "EOF") {
              quitRepl(0);
            } else {
              // e.g. this happens when we have deno.close(3).
              // We want to display the problem.
              const formattedError = formatError(core.errorToJSON(err));
              replError(formattedError);
              quitRepl(1);
            }
          }
        }
      } else {
        console.log("output", output);
        const outputCode = output["<eval>.js"];
        const outputLines = outputCode.split("\n");
        const difference = diff(previousCompiledCode, outputLines);
        previousCompiledCode = outputLines;

        const toEval = difference
          .filter((result: DiffResult<string>): boolean => {
            return result.type === DiffType.added;
          })
          .map((result: DiffResult<string>): string => {
            return result.value;
          })
          .join("\n");

        evaluate(toEval);
        break;
      }
    }
  }
}
