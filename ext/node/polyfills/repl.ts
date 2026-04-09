// Copyright 2018-2026 the Deno authors. MIT license.
// Copyright Joyent and Node contributors. All rights reserved. MIT license.

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file prefer-primordials no-this-alias no-unused-vars no-explicit-any

import { primordials } from "ext:core/mod.js";
const { Symbol } = primordials;

import { Interface } from "ext:deno_node/_readline.mjs";
import { commonPrefix } from "ext:deno_node/internal/readline/utils.mjs";
import { inspect } from "ext:deno_node/internal/util/inspect.mjs";
import {
  ERR_INVALID_REPL_EVAL_CONFIG,
  ERR_MISSING_ARGS,
} from "ext:deno_node/internal/errors.ts";
import { validateFunction } from "ext:deno_node/internal/validators.mjs";
import vm from "node:vm";
import process from "node:process";
import path from "node:path";
import fs from "node:fs";
import { Console } from "node:console";
import Module from "node:module";
import EventEmitter from "node:events";

export const REPL_MODE_SLOPPY = Symbol("repl-sloppy");
export const REPL_MODE_STRICT = Symbol("repl-strict");

const kBufferedCommandSymbol = Symbol("bufferedCommand");
const kLoadingSymbol = Symbol("loading");
const kContextId = Symbol("contextId");
const kStandaloneREPL = Symbol("standaloneREPL");

// Multiline prompt indicator
const kMultilinePrompt = "... ";

// This is the default "writer" value
const writer = (obj: unknown) => inspect(obj, writer.options);
writer.options = { ...inspect.defaultOptions, showProxy: true };

export class Recoverable extends SyntaxError {
  err: Error;
  constructor(err: Error) {
    super();
    this.err = err;
  }
}

/**
 * Check if code looks like it might be an object literal
 * that needs to be wrapped in parens to eval as an expression.
 */
function isObjectLiteral(code: string): boolean {
  const trimmed = code.trim();
  return trimmed.startsWith("{") && !trimmed.startsWith("{\\");
}

/**
 * Check if a syntax error is recoverable (i.e., the user might continue typing).
 */
function isRecoverableError(e: unknown, code: string): boolean {
  if (!e || typeof e !== "object" || (e as Error).name !== "SyntaxError") {
    return false;
  }
  const message = (e as Error).message;

  // Check for common recoverable patterns
  if (/Unexpected end of input/.test(message)) return true;
  if (/Unexpected token/.test(message)) {
    // Check if the code has unbalanced braces/brackets/parens
    let depth = 0;
    for (let i = 0; i < code.length; i++) {
      const c = code[i];
      if (c === "{" || c === "(" || c === "[") depth++;
      else if (c === "}" || c === ")" || c === "]") depth--;
    }
    if (depth > 0) return true;
  }
  if (/Unterminated string/.test(message)) return true;
  if (/Unterminated template/.test(message)) return true;

  return false;
}

function _turnOnEditorMode(repl: REPLServer) {
  repl.editorMode = true;
  Interface.prototype.setPrompt.call(repl, "");
}

function _turnOffEditorMode(repl: REPLServer) {
  repl.editorMode = false;
  repl.setPrompt(repl._initialPrompt);
}

function defineDefaultCommands(repl: REPLServer) {
  repl.defineCommand("break", {
    help: "Sometimes you get stuck, this gets you out",
    action: function (this: REPLServer) {
      this.clearBufferedCommand();
      this.displayPrompt();
    },
  });

  let clearMessage: string;
  if (repl.useGlobal) {
    clearMessage = "Alias for .break";
  } else {
    clearMessage = "Break, and also clear the local context";
  }
  repl.defineCommand("clear", {
    help: clearMessage,
    action: function (this: REPLServer) {
      this.clearBufferedCommand();
      if (!this.useGlobal) {
        this.output.write("Clearing context...\n");
        this.resetContext();
      }
      this.displayPrompt();
    },
  });

  repl.defineCommand("exit", {
    help: "Exit the REPL",
    action: function (this: REPLServer) {
      this.close();
    },
  });

  repl.defineCommand("help", {
    help: "Print this help message",
    action: function (this: REPLServer) {
      const names = Object.keys(this.commands).sort();
      const longestNameLength = Math.max(
        ...names.map((name) => name.length),
      );
      names.forEach((name) => {
        const cmd = this.commands[name];
        const spaces = " ".repeat(longestNameLength - name.length + 3);
        const line = `.${name}${cmd.help ? spaces + cmd.help : ""}\n`;
        this.output.write(line);
      });
      this.output.write(
        "\nPress Ctrl+C to abort current expression, " +
          "Ctrl+D to exit the REPL\n",
      );
      this.displayPrompt();
    },
  });

  repl.defineCommand("save", {
    help: "Save all evaluated commands in this REPL session to a file",
    action: function (this: REPLServer, file: string) {
      try {
        if (file === "") {
          throw new ERR_MISSING_ARGS("file");
        }
        fs.writeFileSync(file, this.lines.join("\n"));
        this.output.write(`Session saved to: ${file}\n`);
      } catch (error) {
        if ((error as { code?: string })?.code === "ERR_MISSING_ARGS") {
          this.output.write(`${(error as Error).message}\n`);
        } else {
          this.output.write(`Failed to save: ${file}\n`);
        }
      }
      this.displayPrompt();
    },
  });

  repl.defineCommand("load", {
    help: "Load JS from a file into the REPL session",
    action: function (this: REPLServer, file: string) {
      try {
        if (file === "") {
          throw new ERR_MISSING_ARGS("file");
        }
        const stats = fs.statSync(file);
        if (stats && stats.isFile()) {
          _turnOnEditorMode(this);
          this[kLoadingSymbol] = true;
          const data = fs.readFileSync(file, "utf8");
          this.write(data);
          this[kLoadingSymbol] = false;
          _turnOffEditorMode(this);
          this.write("\n");
        } else {
          this.output.write(
            `Failed to load: ${file} is not a valid file\n`,
          );
        }
      } catch (error) {
        if ((error as { code?: string })?.code === "ERR_MISSING_ARGS") {
          this.output.write(`${(error as Error).message}\n`);
        } else {
          this.output.write(`Failed to load: ${file}\n`);
        }
      }
      this.displayPrompt();
    },
  });

  repl.defineCommand("editor", {
    help: "Enter editor mode",
    action: function (this: REPLServer) {
      if (!this.terminal) {
        this.displayPrompt();
        return;
      }
      _turnOnEditorMode(this);
      this.output.write(
        "// Entering editor mode (Ctrl+D to finish, Ctrl+C to cancel)\n",
      );
    },
  });
}

function _memory(this: REPLServer, cmd: string) {
  this.lines = this.lines || [];
  this.lines.level = this.lines.level || [];

  if (cmd) {
    const len = this.lines.level.length ? this.lines.level.length - 1 : 0;
    this.lines.push("  ".repeat(len) + cmd);
  } else {
    this.lines.push("");
  }

  if (!cmd) {
    this.lines.level = [];
    return;
  }

  const countMatches = (regex: RegExp, str: string) => {
    let count = 0;
    while (regex.exec(str) !== null) count++;
    return count;
  };

  const dw = countMatches(/[{(]/g, cmd);
  const up = countMatches(/[})]/g, cmd);
  let depth = dw - up;

  if (depth) {
    (function workIt() {
      const self = this as REPLServer;
      if (depth > 0) {
        self.lines.level.push({
          line: self.lines.length - 1,
          depth: depth,
        });
      } else if (depth < 0) {
        const curr = self.lines.level.pop();
        if (curr) {
          const tmp = curr.depth + depth;
          if (tmp < 0) {
            depth += curr.depth;
            workIt.call(self);
          } else if (tmp > 0) {
            curr.depth += depth;
            self.lines.level.push(curr);
          }
        }
      }
    }).call(this);
  }
}

type REPLCommand = {
  help?: string;
  action: (this: REPLServer, ...args: any[]) => void;
};

export class REPLServer extends (Interface as any) {
  constructor(
    prompt?: any,
    stream?: any,
    eval_?: any,
    useGlobal?: boolean,
    ignoreUndefined?: boolean,
    replMode?: symbol,
  ) {
    let options: Record<string, unknown>;
    if (prompt !== null && typeof prompt === "object") {
      options = { ...prompt };
      stream = options.stream || options.socket;
      eval_ = options.eval;
      useGlobal = options.useGlobal as boolean | undefined;
      ignoreUndefined = options.ignoreUndefined as boolean | undefined;
      prompt = options.prompt;
      replMode = options.replMode as symbol | undefined;
    } else {
      options = {};
    }

    if (!options.input && !options.output) {
      stream = stream || process;
      options.input = stream.stdin || stream;
      options.output = stream.stdout || stream;
    }

    if (options.terminal === undefined) {
      options.terminal = !!(options.output as { isTTY?: boolean })?.isTTY;
    }
    options.terminal = !!options.terminal;

    if (options.terminal && options.useColors === undefined) {
      // Check if the output stream supports colors
      options.useColors = !!(options.output as { isTTY?: boolean })?.isTTY;
    }

    // Default prompt
    if (prompt === undefined) {
      prompt = "> ";
    }

    super({
      input: options.input,
      output: options.output,
      completer: options.completer || completer,
      terminal: options.terminal,
      historySize: options.historySize,
      prompt,
    });

    // Create a fake domain for compatibility. Node uses domains for error
    // handling, but we use a simpler EventEmitter-based approach.
    this._domain = new EventEmitter();

    // Deprecated inputStream/outputStream properties (DEP0141)
    Object.defineProperty(this, "inputStream", {
      get: () => this.input,
      set: (val: any) => {
        this.input = val;
      },
      enumerable: false,
      configurable: true,
    });
    Object.defineProperty(this, "outputStream", {
      get: () => this.output,
      set: (val: any) => {
        this.output = val;
      },
      enumerable: false,
      configurable: true,
    });

    this.allowBlockingCompletions = !!options.allowBlockingCompletions;
    this.useColors = !!options.useColors;
    this._isStandalone =
      !!(options as Record<symbol, unknown>)[kStandaloneREPL];
    this.useGlobal = !!useGlobal;
    this.ignoreUndefined = !!ignoreUndefined;
    this.replMode = replMode || REPL_MODE_SLOPPY;
    this.underscoreAssigned = false;
    this.last = undefined;
    this.underscoreErrAssigned = false;
    this.lastError = undefined;
    this.breakEvalOnSigint = !!options.breakEvalOnSigint;
    this.editorMode = false;
    this[kContextId] = undefined;
    this._initialPrompt = prompt as string;

    if (this.breakEvalOnSigint && eval_) {
      throw new ERR_INVALID_REPL_EVAL_CONFIG();
    }

    if ((options as Record<symbol, unknown>)[kStandaloneREPL]) {
      _module.exports.repl = this;
    }

    eval_ = eval_ || defaultEval;

    const self = this;

    // Pause taking in new input, and store the keys in a buffer.

    const pausedBuffer: any[] = [];
    let paused = false;
    function pause() {
      paused = true;
    }

    function unpause() {
      if (!paused) return;
      paused = false;

      let entry: any;
      const tmpCompletionEnabled = self.isCompletionEnabled;
      while ((entry = pausedBuffer.shift()) !== undefined) {
        const [type, payload, isCompletionEnabled] = entry;
        switch (type) {
          case "key": {
            const [d, key] = payload;
            self.isCompletionEnabled = isCompletionEnabled;
            self._ttyWrite(d, key);
            break;
          }
          case "close":
            self.emit("exit");
            break;
        }
        if (paused) {
          break;
        }
      }
      self.isCompletionEnabled = tmpCompletionEnabled;
    }

    function defaultEval(
      code: string,
      context: any,
      _file: string,
      cb: (err: Error | null, result?: any) => void,
    ) {
      let result;
      let err: Error | null = null;
      let wrappedCmd = false;
      const input = code;

      if (isObjectLiteral(code)) {
        try {
          // Try to parse as-is first
          new vm.Script(code);
        } catch {
          // If it fails, try wrapping in parens
          code = `(${code.trim()})\n`;
          wrappedCmd = true;
        }
      }

      // Empty input
      if (code === "\n") return cb(null);

      if (err === null) {
        let wrappedErr: Error | undefined;
        while (true) {
          try {
            if (
              self.replMode === REPL_MODE_STRICT &&
              !/^\s*$/.test(code)
            ) {
              code = `'use strict'; void 0;\n${code}`;
            }
            const script = new vm.Script(code, { filename: _file });
            if (self.useGlobal) {
              result = script.runInThisContext({ displayErrors: false });
            } else {
              result = script.runInContext(context, {
                displayErrors: false,
              });
            }
          } catch (e) {
            if (wrappedCmd) {
              wrappedCmd = false;
              code = input;
              wrappedErr = e as Error;
              continue;
            }
            const error = wrappedErr || e;
            if (isRecoverableError(error as Error, code)) {
              err = new Recoverable(error as Error);
            } else {
              err = error as Error;
            }
          }
          break;
        }
      }

      cb(err, result);
    }

    self.eval = function REPLEval(
      code: string,
      context: any,
      file: string,
      cb: (err: Error | null, result?: any) => void,
    ) {
      eval_(code, context, file, cb);
    };

    self.clearBufferedCommand();

    function completer(text: string, cb: any) {
      const callback = self.editorMode ? self.completeOnEditorMode(cb) : cb;
      // Return empty completions by default
      callback(null, [[], text]);
    }

    self.resetContext();

    this.commands = Object.create(null);
    defineDefaultCommands(this);

    // Figure out which "writer" function to use
    self.writer = options.writer || writer;

    if (self.writer === writer) {
      writer.options.colors = self.useColors;
    }

    function _parseREPLKeyword(
      this: REPLServer,
      keyword: string,
      rest: string,
    ): boolean {
      const cmd = this.commands[keyword];
      if (cmd) {
        cmd.action.call(this, rest);
        return true;
      }
      return false;
    }

    self.on("close", function emitExit() {
      if (paused) {
        pausedBuffer.push(["close"]);
        return;
      }
      self.emit("exit");
    });

    let sawSIGINT = false;
    let sawCtrlD = false;
    self.on("SIGINT", function onSigInt() {
      const empty = self.line.length === 0;
      self.clearLine();
      _turnOffEditorMode(self);

      const cmd = self[kBufferedCommandSymbol];
      if (!(cmd && cmd.length > 0) && empty) {
        if (sawSIGINT) {
          self.close();
          sawSIGINT = false;
          return;
        }
        self.output.write(
          "(To exit, press Ctrl+C again or Ctrl+D or type .exit)\n",
        );
        sawSIGINT = true;
      } else {
        sawSIGINT = false;
      }

      self.clearBufferedCommand();
      self.lines.level = [];
      self.displayPrompt();
    });

    self.on("line", function onLine(cmd: string) {
      cmd = cmd || "";
      sawSIGINT = false;

      if (self.editorMode) {
        self[kBufferedCommandSymbol] += cmd + "\n";

        // code alignment
        const matches = self._sawKeyPress && !self[kLoadingSymbol]
          ? /^\s+/.exec(cmd)
          : null;
        if (matches) {
          const prefix = matches[0];
          self.write(prefix);
          self.line = prefix;
          self.cursor = prefix.length;
        }
        _memory.call(self, cmd);
        return;
      }

      // Check REPL keywords and empty lines against a trimmed line input.
      const trimmedCmd = cmd.trim();

      if (trimmedCmd) {
        if (
          trimmedCmd.charAt(0) === "." &&
          trimmedCmd.charAt(1) !== "." &&
          Number.isNaN(Number.parseFloat(trimmedCmd))
        ) {
          const matches = /^\.([^\s]+)\s*(.*)$/.exec(trimmedCmd);
          const keyword = matches?.[1];
          const rest = matches?.[2];
          if (
            keyword &&
            _parseREPLKeyword.call(self, keyword, rest || "") === true
          ) {
            return;
          }
          if (!self[kBufferedCommandSymbol]) {
            self.output.write("Invalid REPL keyword\n");
            finish(null);
            return;
          }
        }
      }

      const evalCmd = self[kBufferedCommandSymbol] + cmd + "\n";

      self.eval(evalCmd, self.context, "repl", finish);

      function finish(e: Error | null, ret?: any) {
        _memory.call(self, cmd);

        if (
          e &&
          !self[kBufferedCommandSymbol] &&
          cmd.trim().startsWith("npm ") &&
          !(e instanceof Recoverable)
        ) {
          self.output.write(
            "npm should be run outside of the " +
              "Node.js REPL, in your normal shell.\n" +
              "(Press Ctrl+D to exit.)\n",
          );
          self.displayPrompt();
          return;
        }

        // If error was SyntaxError and not JSON.parse error
        if (e instanceof Recoverable && !sawCtrlD) {
          // Start multiline
          self[kBufferedCommandSymbol] += cmd + "\n";
          self.displayPrompt();
          return;
        }

        if (e) {
          self._handleError((e as Recoverable).err || e);
        }

        // Clear buffer if no SyntaxErrors
        self.clearBufferedCommand();
        sawCtrlD = false;

        // If we got any output - print it (if no error)
        if (
          !e &&
          arguments.length === 2 &&
          (!self.ignoreUndefined || ret !== undefined)
        ) {
          if (!self.underscoreAssigned) {
            self.last = ret;
          }
          self.output.write(self.writer(ret) + "\n");
        }

        if (!self.closed && !e) {
          self.displayPrompt();
        }
      }
    });

    self.on("SIGCONT", function onSigCont() {
      if (self.editorMode) {
        self.output.write(`${self._initialPrompt}.editor\n`);
        self.output.write(
          "// Entering editor mode (Ctrl+D to finish, Ctrl+C to cancel)\n",
        );
        self.output.write(`${self[kBufferedCommandSymbol]}\n`);
        self.prompt(true);
      } else {
        self.displayPrompt(true);
      }
    });

    // Wrap readline tty to enable editor mode and pausing.
    const ttyWrite = self._ttyWrite.bind(self);

    self._ttyWrite = (d: any, key: any) => {
      key = key || {};
      if (
        paused &&
        !(self.breakEvalOnSigint && key.ctrl && key.name === "c")
      ) {
        pausedBuffer.push([
          "key",
          [d, key],
          self.isCompletionEnabled,
        ]);
        return;
      }
      if (!self.editorMode || !self.terminal) {
        // Before exiting, make sure to clear the line.
        if (
          key.ctrl &&
          key.name === "d" &&
          self.cursor === 0 &&
          self.line.length === 0
        ) {
          self.clearLine();
        }
        ttyWrite(d, key);
        return;
      }

      // Editor mode
      if (key.ctrl && !key.shift) {
        switch (key.name) {
          case "d": // End editor mode
            _turnOffEditorMode(self);
            sawCtrlD = true;
            ttyWrite(d, { name: "return" });
            break;
          case "n": // Override next history item
          case "p": // Override previous history item
            break;
          default:
            ttyWrite(d, key);
        }
      } else {
        switch (key.name) {
          case "up": // Override previous history item
          case "down": // Override next history item
            break;
          case "tab":
            // Prevent double tab behavior
            self._previousKey = null;
            ttyWrite(d, key);
            break;
          default:
            ttyWrite(d, key);
        }
      }
    };

    self.displayPrompt();
  }

  setupHistory(
    _historyFile?: string,
    cb?: (err: Error | null, repl: REPLServer) => void,
  ) {
    if (typeof cb === "function") {
      cb(null, this);
    }
  }

  clearBufferedCommand() {
    this[kBufferedCommandSymbol] = "";
  }

  _handleError(e: Error) {
    let errStack = "";

    if (typeof e === "object" && e !== null) {
      const isError = e instanceof Error ||
        (typeof (e as any).name === "string" &&
          typeof (e as any).stack === "string");
      if (isError && (e as any).stack) {
        if (e.name === "SyntaxError") {
          // Remove stack trace
          errStack = e.stack
            .replace(/^\s+at\s.*\n?/gm, "")
            .replace(/^REPL\d+:\d+\r?\n/, "");
        } else {
          // For non-syntax errors, strip ALL stack frames.
          // Node uses overrideStackTrace to filter internal frames;
          // we simply remove all "at ..." lines for REPL errors.
          errStack = e.stack.replace(/\n\s+at\s.*/g, "");
        }
      }

      if (!errStack) {
        errStack = this.writer(e);
      }

      // Remove one line error braces to keep the old style in place.
      if (errStack[0] === "[" && errStack[errStack.length - 1] === "]") {
        errStack = errStack.slice(1, -1);
      }
    }

    if (!this.underscoreErrAssigned) {
      this.lastError = e;
    }

    if (errStack === "") {
      errStack = this.writer(e);
    }

    const lines = errStack.split(/(?<=\n)/);
    let matched = false;

    errStack = "";
    lines.forEach((line: string) => {
      if (
        !matched &&
        /^\[?([A-Z][a-z0-9_]*)*Error/.test(line)
      ) {
        errStack += writer.options.breakLength >= line.length
          ? `Uncaught ${line}`
          : `Uncaught:\n${line}`;
        matched = true;
      } else {
        errStack += line;
      }
    });
    if (!matched) {
      const ln = lines.length === 1 ? " " : ":\n";
      errStack = `Uncaught${ln}${errStack}`;
    }
    // Normalize line endings.
    errStack += errStack.endsWith("\n") ? "" : "\n";
    this.output.write(errStack);
    this.clearBufferedCommand();
    this.lines.level = [];
    if (!this.closed) {
      this.displayPrompt();
    }
  }

  close() {
    if (this.closed || this._closing) return;
    this._closing = true;
    const self = this;
    process.nextTick(() => {
      try {
        // @ts-ignore - calling parent close
        Interface.prototype.close.call(self);
      } catch {
        // May fail if input stream already destroyed
      }
      self._closing = false;
    });
  }

  createContext() {
    let context: any;
    if (this.useGlobal) {
      context = globalThis;
    } else {
      context = vm.createContext();
      context.global = context;
      let _console;
      try {
        _console = new Console(this.output);
      } catch {
        // If Console constructor fails (e.g., non-standard stream),
        // fall back to the global console.
        _console = console;
      }
      Object.defineProperty(context, "console", {
        configurable: true,
        writable: true,
        value: _console,
      });
    }

    // Set up module and require in the context
    try {
      const replRequire = Module.createRequire(
        path.join(process.cwd(), "repl"),
      );
      Object.defineProperty(context, "require", {
        configurable: true,
        writable: true,
        value: replRequire,
      });
    } catch {
      // createRequire may fail in some environments
    }

    Object.defineProperty(context, "module", {
      configurable: true,
      writable: true,
      value: { exports: {} },
    });

    return context;
  }

  resetContext() {
    this.context = this.createContext();
    this.underscoreAssigned = false;
    this.underscoreErrAssigned = false;
    this.lines = [];
    this.lines.level = [];

    Object.defineProperty(this.context, "_", {
      configurable: true,
      get: () => this.last,
      set: (value) => {
        this.last = value;
        if (!this.underscoreAssigned) {
          this.underscoreAssigned = true;
          this.output.write("Expression assignment to _ now disabled.\n");
        }
      },
    });

    Object.defineProperty(this.context, "_error", {
      configurable: true,
      get: () => this.lastError,
      set: (value) => {
        this.lastError = value;
        if (!this.underscoreErrAssigned) {
          this.underscoreErrAssigned = true;
          this.output.write(
            "Expression assignment to _error now disabled.\n",
          );
        }
      },
    });

    // Allow REPL extensions to extend the new context
    this.emit("reset", this.context);
  }

  displayPrompt(preserveCursor?: boolean) {
    let prompt = this._initialPrompt;
    if (this[kBufferedCommandSymbol].length) {
      prompt = kMultilinePrompt;
    }

    super.setPrompt(prompt);
    this.prompt(preserveCursor);
  }

  setPrompt(prompt: string) {
    this._initialPrompt = prompt;
    super.setPrompt(prompt);
  }

  complete(...args: any[]) {
    Reflect.apply(this.completer, this, args);
  }

  completeOnEditorMode(callback: any) {
    return (err: Error | null, results: any) => {
      if (err) return callback(err);

      const [completions, completeOn = ""] = results;
      let result = completions.filter(Boolean);

      if (completeOn && result.length !== 0) {
        result = [commonPrefix(result)];
      }

      callback(null, [result, completeOn]);
    };
  }

  defineCommand(
    keyword: string,
    cmd: REPLCommand | ((this: REPLServer, ...args: unknown[]) => void),
  ) {
    if (typeof cmd === "function") {
      cmd = { action: cmd };
    } else {
      validateFunction(cmd.action, "cmd.action");
    }
    this.commands[keyword] = cmd;
  }
}

function completer(this: REPLServer, _text: string, cb: any) {
  cb(null, [[], _text]);
}

export function start(
  prompt?: any,
  source?: any,
  eval_?: any,
  useGlobal?: boolean,
  ignoreUndefined?: boolean,
  replMode?: symbol,
) {
  return new REPLServer(
    prompt,
    source,
    eval_,
    useGlobal,
    ignoreUndefined,
    replMode,
  );
}

export const builtinModules = [
  "assert",
  "async_hooks",
  "buffer",
  "child_process",
  "cluster",
  "console",
  "constants",
  "crypto",
  "dgram",
  "diagnostics_channel",
  "dns",
  "domain",
  "events",
  "fs",
  "http",
  "http2",
  "https",
  "inspector",
  "module",
  "net",
  "os",
  "path",
  "perf_hooks",
  "process",
  "punycode",
  "querystring",
  "readline",
  "repl",
  "stream",
  "string_decoder",
  "sys",
  "timers",
  "tls",
  "trace_events",
  "tty",
  "url",
  "util",
  "v8",
  "vm",
  "wasi",
  "worker_threads",
  "zlib",
];

export const _builtinLibs = builtinModules;

// Module-level reference for standalone REPL tracking
const _module = { exports: {} as Record<string, unknown> };

export default {
  REPLServer,
  builtinModules,
  _builtinLibs,
  start,
  writer,
  REPL_MODE_SLOPPY,
  REPL_MODE_STRICT,
  Recoverable,
};
