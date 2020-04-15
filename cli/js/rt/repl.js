System.register(
  "$deno$/repl.ts",
  [
    "$deno$/ops/os.ts",
    "$deno$/core.ts",
    "$deno$/web/console.ts",
    "$deno$/ops/repl.ts",
    "$deno$/ops/resources.ts",
  ],
  function (exports_104, context_104) {
    "use strict";
    let os_ts_3,
      core_ts_8,
      console_ts_6,
      repl_ts_1,
      resources_ts_8,
      helpMsg,
      replCommands,
      recoverableErrorMessages,
      lastEvalResult,
      lastThrownError;
    const __moduleName = context_104 && context_104.id;
    function replLog(...args) {
      core_ts_8.core.print(console_ts_6.stringifyArgs(args) + "\n");
    }
    function replError(...args) {
      core_ts_8.core.print(console_ts_6.stringifyArgs(args) + "\n", true);
    }
    function isRecoverableError(e) {
      return recoverableErrorMessages.includes(e.message);
    }
    // Evaluate code.
    // Returns true if code is consumed (no error/irrecoverable error).
    // Returns false if error is recoverable
    function evaluate(code) {
      const [result, errInfo] = core_ts_8.core.evalContext(code);
      if (!errInfo) {
        lastEvalResult = result;
        replLog(result);
      } else if (errInfo.isCompileError && isRecoverableError(errInfo.thrown)) {
        // Recoverable compiler error
        return false; // don't consume code.
      } else {
        lastThrownError = errInfo.thrown;
        if (errInfo.isNativeError) {
          const formattedError = core_ts_8.core.formatError(errInfo.thrown);
          replError(formattedError);
        } else {
          replError("Thrown:", errInfo.thrown);
        }
      }
      return true;
    }
    // @internal
    async function replLoop() {
      const { console } = globalThis;
      Object.defineProperties(globalThis, replCommands);
      const historyFile = "deno_history.txt";
      const rid = repl_ts_1.startRepl(historyFile);
      const quitRepl = (exitCode) => {
        // Special handling in case user calls deno.close(3).
        try {
          resources_ts_8.close(rid); // close signals Drop on REPL and saves history.
        } catch {}
        os_ts_3.exit(exitCode);
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
      while (true) {
        let code = "";
        // Top level read
        try {
          code = await repl_ts_1.readline(rid, "> ");
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
              const formattedError = core_ts_8.core.formatError(err);
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
            code += await repl_ts_1.readline(rid, "  ");
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
              const formattedError = core_ts_8.core.formatError(err);
              replError(formattedError);
              quitRepl(1);
            }
          }
        }
      }
    }
    exports_104("replLoop", replLoop);
    return {
      setters: [
        function (os_ts_3_1) {
          os_ts_3 = os_ts_3_1;
        },
        function (core_ts_8_1) {
          core_ts_8 = core_ts_8_1;
        },
        function (console_ts_6_1) {
          console_ts_6 = console_ts_6_1;
        },
        function (repl_ts_1_1) {
          repl_ts_1 = repl_ts_1_1;
        },
        function (resources_ts_8_1) {
          resources_ts_8 = resources_ts_8_1;
        },
      ],
      execute: function () {
        helpMsg = [
          "_       Get last evaluation result",
          "_error  Get last thrown error",
          "exit    Exit the REPL",
          "help    Print this help message",
        ].join("\n");
        replCommands = {
          exit: {
            get() {
              os_ts_3.exit(0);
            },
          },
          help: {
            get() {
              return helpMsg;
            },
          },
        };
        // Error messages that allow users to continue input
        // instead of throwing an error to REPL
        // ref: https://github.com/v8/v8/blob/master/src/message-template.h
        // TODO(kevinkassimo): this list might not be comprehensive
        recoverableErrorMessages = [
          "Unexpected end of input",
          "Missing initializer in const declaration",
          "Missing catch or finally after try",
          "missing ) after argument list",
          "Unterminated template literal",
        ];
        lastEvalResult = undefined;
        lastThrownError = undefined;
      },
    };
  }
);
