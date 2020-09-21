// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

((window) => {
  const core = window.Deno.core;
  const exit = window.__bootstrap.os.exit;
  const version = window.__bootstrap.version.version;
  const inspectArgs = window.__bootstrap.console.inspectArgs;

  function opStartRepl(historyFile) {
    return core.jsonOpSync("op_repl_start", { historyFile });
  }

  function opReadline(rid, prompt) {
    return core.jsonOpAsync("op_repl_readline", { rid, prompt });
  }

  function replLog(...args) {
    core.print(inspectArgs(args) + "\n");
  }

  function replError(...args) {
    core.print(inspectArgs(args) + "\n", true);
  }

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

  function isRecoverableError(e) {
    return recoverableErrorMessages.includes(e.message);
  }

  // Returns `true` if `close()` is called in REPL.
  // We should quit the REPL when this function returns `true`.
  function isCloseCalled() {
    return globalThis.closed;
  }

  let lastEvalResult = undefined;
  let lastThrownError = undefined;

  // Evaluate code.
  // Returns true if code is consumed (no error/irrecoverable error).
  // Returns false if error is recoverable
  function evaluate(code, preprocess = true) {
    const rawCode = code;
    if (preprocess) {
      // It is a bit unexpected that { "foo": "bar" } is interpreted as a block
      // statement rather than an object literal so we interpret it as an expression statement
      // to match the behavior found in a typical prompt including browser developer tools.
      if (code.trimLeft().startsWith("{") && !code.trimRight().endsWith(";")) {
        code = `(${code})`;
      }
    }

    // each evalContext is a separate function body, and we want strict mode to
    // work, so we should ensure that the code starts with "use strict"
    const [result, errInfo] = core.evalContext(`"use strict";\n\n${code}`);

    if (!errInfo) {
      // when a function is eval'ed with just "use strict" sometimes the result
      // is "use strict" which should be discarded
      lastEvalResult = typeof result === "string" && result === "use strict"
        ? undefined
        : result;
      if (!isCloseCalled()) {
        replLog("%o", lastEvalResult);
      }
    } else if (errInfo.isCompileError && code.length != rawCode.length) {
      return evaluate(rawCode, false);
    } else if (errInfo.isCompileError && isRecoverableError(errInfo.thrown)) {
      // Recoverable compiler error
      return false; // don't consume code.
    } else {
      lastThrownError = errInfo.thrown;
      if (errInfo.isNativeError) {
        const formattedError = core.formatError(errInfo.thrown);
        replError(formattedError);
      } else {
        replError("Thrown:", errInfo.thrown);
      }
    }
    return true;
  }

  async function replLoop() {
    const { console } = globalThis;

    const historyFile = "deno_history.txt";
    const rid = opStartRepl(historyFile);

    const quitRepl = (exitCode) => {
      // Special handling in case user calls deno.close(3).
      try {
        core.close(rid); // close signals Drop on REPL and saves history.
      } catch {}
      exit(exitCode);
    };

    // Configure globalThis._ to give the last evaluation result.
    Object.defineProperty(globalThis, "_", {
      configurable: true,
      get: () => lastEvalResult,
      set: (value) => {
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
      get: () => lastThrownError,
      set: (value) => {
        Object.defineProperty(globalThis, "_error", {
          value: value,
          writable: true,
          enumerable: true,
          configurable: true,
        });
        console.log("Last thrown error is no longer saved to _error.");
      },
    });

    replLog(`Deno ${version.deno}`);
    replLog("exit using ctrl+d or close()");

    while (true) {
      if (isCloseCalled()) {
        quitRepl(0);
      }

      let code = "";
      // Top level read
      try {
        code = await opReadline(rid, "> ");
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
          code += await opReadline(rid, "  ");
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

  window.__bootstrap.repl = {
    replLoop,
  };
})(this);
