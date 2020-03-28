// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { exit } from "./ops/os.ts";
import { core } from "./core.ts";
import { stringifyArgs } from "./web/console.ts";
import { startRepl, readline } from "./ops/repl.ts";
import { close } from "./ops/resources.ts";

function replLog(...args: unknown[]): void {
  core.print(stringifyArgs(args) + "\n");
}

function replError(...args: unknown[]): void {
  core.print(stringifyArgs(args) + "\n", true);
}

const helpMsg = [
  "_       Get last evaluation result",
  "_error  Get last thrown error",
  "exit    Exit the REPL",
  "help    Print this help message",
].join("\n");

const replCommands = {
  exit: {
    get(): void {
      exit(0);
    },
  },
  help: {
    get(): string {
      return helpMsg;
    },
  },
};

// Error messages that allow users to continue input
// instead of throwing an error to REPL
// ref: https://github.com/v8/v8/blob/master/src/message-template.h
// TODO(kevinkassimo): this list might not be comprehensive
const recoverableErrorMessages = [
  "Unexpected end of input", // { or [ or (
  "Missing initializer in const declaration", // const a
  "Missing catch or finally after try", // try {}
  "missing ) after argument list", // console.log(1
  "Unterminated template literal", // `template
  // TODO(kevinkassimo): need a parser to handling errors such as:
  // "Missing } in template expression" // `${ or `${ a 123 }`
];

function isRecoverableError(e: Error): boolean {
  return recoverableErrorMessages.includes(e.message);
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
type Value = any;

let lastEvalResult: Value = undefined;
let lastThrownError: Value = undefined;

// Evaluate code.
// Returns true if code is consumed (no error/irrecoverable error).
// Returns false if error is recoverable
function evaluate(code: string): boolean {
  const [result, errInfo] = core.evalContext(code);
  if (!errInfo) {
    lastEvalResult = result;
    replLog(result);
  } else if (errInfo.isCompileError && isRecoverableError(errInfo.thrown)) {
    // Recoverable compiler error
    return false; // don't consume code.
  } else {
    lastThrownError = errInfo.thrown;
    if (errInfo.isNativeError) {
      const formattedError = core.formatError(errInfo.thrown as Error);
      replError(formattedError);
    } else {
      replError("Thrown:", errInfo.thrown);
    }
  }
  return true;
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
        configurable: true,
      });
      console.log("Last evaluation result is no longer saved to _.");
    },
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
        configurable: true,
      });
      console.log("Last thrown error is no longer saved to _error.");
    },
  });

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
          const formattedError = core.formatError(err);
          replError(formattedError);
        }
        // Quit REPL anyways.
        quitRepl(1);
      }
    }
    // Start continued read
    while (!evaluate(code)) {
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
          const formattedError = core.formatError(err);
          replError(formattedError);
          quitRepl(1);
        }
      }
    }
  }
}
