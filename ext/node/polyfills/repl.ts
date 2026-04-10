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

// ANSI cursor control helpers for preview
function _cursorTo(stream: any, x: number) {
  stream.write(`\x1b[${x + 1}G`);
}

function _moveCursor(stream: any, dx: number, dy: number) {
  let data = "";
  if (dx < 0) data += `\x1b[${-dx}D`;
  else if (dx > 0) data += `\x1b[${dx}C`;
  if (dy < 0) data += `\x1b[${-dy}A`;
  else if (dy > 0) data += `\x1b[${dy}B`;
  if (data) stream.write(data);
}

function _clearLine(stream: any, dir?: number) {
  if (dir !== undefined && dir < 0) stream.write("\x1b[1K");
  else if (dir !== undefined && dir > 0) stream.write("\x1b[0K");
  else stream.write("\x1b[2K");
}

function _getPropertyNames(obj: any): string[] {
  if (!obj) return [];
  try {
    return Object.getOwnPropertyNames(obj).filter((name: string) => {
      return /^[a-zA-Z_$][\w$]*$/.test(name);
    });
  } catch {
    return [];
  }
}

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
    const usePreview = !!options.terminal && options.preview !== false;

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

      // Empty input
      if (code === "\n") return cb(null);

      // If it looks like an object literal (starts with { and no trailing ;),
      // try wrapping in parens first to treat as expression.
      // This matches Node.js behavior: wrap first, fallback to unwrapped.
      if (
        isObjectLiteral(code) &&
        !/;\s*$/.test(code.trim())
      ) {
        code = `(${code.trim()})\n`;
        wrappedCmd = true;
      }

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
              // Wrapped version failed, try original
              wrappedCmd = false;
              code = input;
              wrappedErr = e as Error;
              continue;
            }
            // Use the unwrapped error unless it's a SyntaxError and the
            // wrapped version also had a SyntaxError (in which case we
            // prefer the unwrapped SyntaxError for better messaging).
            const error = e as Error;
            if (isRecoverableError(error, code)) {
              err = new Recoverable(error);
            } else {
              // Attach source context for SyntaxErrors so _handleError
              // can display source lines like Node.js does.
              if (
                error != null && typeof error === "object" &&
                error.name === "SyntaxError"
              ) {
                (error as any)._replSourceCode = input.replace(/\n$/, "");
              }
              err = error;
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
      _doComplete(text, callback);
    }

    function _doComplete(
      line: string,
      callback: (err: Error | null, result: [string[], string]) => void,
    ) {
      const completionGroups: string[][] = [];
      let completeOn = "";
      let filter = "";

      // Handle REPL commands (.break, .clear, etc.)
      const cmdMatch = /^\s*\.(\w*)$/.exec(line);
      if (cmdMatch) {
        completionGroups.push(
          Object.keys(self.commands).map((cmd) => `.${cmd}`),
        );
        completeOn = `.${cmdMatch[1]}`;
        if (cmdMatch[1].length) {
          filter = completeOn;
        }
        completionGroupsLoaded();
        return;
      }

      // Match identifier chains: foo, foo.bar, foo.bar.baz, etc.
      const exprMatch =
        /(?:^|[\s;({\[,!~+\-*/&|^%<>?:=])((?:[a-zA-Z_$][\w$]*\??\.)*(?:[a-zA-Z_$][\w$]*)?)\.?$/
          .exec(line);

      let expr = "";

      if (exprMatch) {
        const matchStr = exprMatch[1];
        completeOn = matchStr;

        if (matchStr.endsWith(".")) {
          expr = matchStr.slice(0, -1);
          filter = "";
        } else if (matchStr.includes(".")) {
          const bits = matchStr.split(".");
          filter = bits.pop()!;
          expr = bits.join(".");
        } else {
          filter = matchStr;
        }
      } else if (line.length === 0) {
        completeOn = "";
        filter = "";
      } else {
        callback(null, [[], line]);
        return;
      }

      if (expr) {
        // Member expression completion
        let chaining = ".";
        let evalExpr = expr;
        if (expr.endsWith("?")) {
          evalExpr = expr.slice(0, -1);
          chaining = "?.";
        }

        const wrappedExpr = `try { ${evalExpr} } catch {}`;
        self.eval(
          wrappedExpr,
          self.context,
          "repl",
          (_e: any, obj: any) => {
            if (obj != null) {
              const memberGroups: string[][] = [];
              try {
                let p;
                if (
                  (typeof obj === "object" && obj !== null) ||
                  typeof obj === "function"
                ) {
                  memberGroups.push(_getPropertyNames(obj));
                  p = Object.getPrototypeOf(obj);
                } else {
                  p = obj.constructor ? obj.constructor.prototype : null;
                }
                let sentinel = 5;
                while (p !== null && sentinel-- > 0) {
                  memberGroups.push(_getPropertyNames(p));
                  p = Object.getPrototypeOf(p);
                }
              } catch {
                // Proxy without getOwnPropertyNames
              }

              if (memberGroups.length) {
                const prefix = expr + chaining;
                for (const group of memberGroups) {
                  completionGroups.push(
                    group.map((m: string) => `${prefix}${m}`),
                  );
                }
                if (filter) {
                  filter = `${prefix}${filter}`;
                }
              }
            }
            completionGroupsLoaded();
          },
        );
      } else {
        // Global completion - walk context prototype chain
        if (self.context) {
          try {
            let obj = self.context;
            let sentinel = 5;
            while (obj !== null && sentinel-- > 0) {
              try {
                completionGroups.push(_getPropertyNames(obj));
              } catch {
                // ignore
              }
              obj = Object.getPrototypeOf(obj);
            }
          } catch {
            // ignore
          }
        }

        // JS keywords
        if (filter !== "") {
          completionGroups.push([
            "async",
            "await",
            "break",
            "case",
            "catch",
            "const",
            "continue",
            "debugger",
            "default",
            "delete",
            "do",
            "else",
            "export",
            "false",
            "finally",
            "for",
            "function",
            "if",
            "import",
            "in",
            "instanceof",
            "let",
            "new",
            "null",
            "return",
            "switch",
            "this",
            "throw",
            "true",
            "try",
            "typeof",
            "undefined",
            "var",
            "void",
            "while",
            "with",
            "yield",
          ]);
        }

        completionGroupsLoaded();
      }

      function completionGroupsLoaded() {
        // Filter by prefix
        if (completionGroups.length && filter) {
          const lowerFilter = filter.toLowerCase();
          const filtered: string[][] = [];
          for (const group of completionGroups) {
            const fg = group.filter((str) =>
              str.toLowerCase().startsWith(lowerFilter)
            );
            if (fg.length) filtered.push(fg);
          }
          completionGroups.length = 0;
          completionGroups.push(...filtered);
        }

        // Deduplicate and collect
        const completions: string[] = [];
        const seen = new Set<string>();
        seen.add("");

        for (const group of completionGroups) {
          group.sort((a, b) => (b > a ? 1 : -1));
          const prevSize = seen.size;
          for (const item of group) {
            if (!seen.has(item)) {
              completions.unshift(item);
              seen.add(item);
            }
          }
          if (seen.size !== prevSize) {
            completions.unshift("");
          }
        }

        if (completions.length > 0 && completions[0] === "") {
          completions.shift();
        }

        callback(null, [completions, completeOn]);
      }
    }

    // Preview state
    let inputPreview: string | null = null;
    let completionPreview: string | null = null;
    let previewCompletionCounter = 0;
    let escaped: string | null = null;

    function _getDisplayPosHelper(str: string) {
      if (typeof self._getDisplayPos === "function") {
        return self._getDisplayPos(str);
      }
      return { rows: 0, cols: str.length };
    }

    function getPreviewPos() {
      const displayPos = _getDisplayPosHelper(
        `${self.getPrompt()}${self.line}`,
      );
      const cursorPos = self.line.length !== self.cursor
        ? (typeof self.getCursorPos === "function"
          ? self.getCursorPos()
          : { rows: 0, cols: self.getPrompt().length + self.cursor })
        : displayPos;
      return { displayPos, cursorPos };
    }

    function isCursorAtInputEnd() {
      return self.cursor === self.line.length;
    }

    function showCompletionPreview(line: string) {
      previewCompletionCounter++;
      const count = previewCompletionCounter;

      self.completer(line, (err: any, data: any) => {
        if (count !== previewCompletionCounter) return;
        if (err) return;

        const [rawCompletions, completeOn] = data;
        if (!rawCompletions || rawCompletions.length === 0) return;

        const completions = rawCompletions.filter(Boolean);
        if (completions.length === 0) return;

        const prefix = commonPrefix(completions);
        if (prefix.length <= completeOn.length) return;

        const suffix = prefix.slice(completeOn.length);
        completionPreview = suffix;

        const result = self.useColors
          ? `\x1b[90m${suffix}\x1b[39m`
          : ` // ${suffix}`;

        const { cursorPos, displayPos } = getPreviewPos();
        if (self.line.length !== self.cursor) {
          _cursorTo(self.output, displayPos.cols);
          _moveCursor(self.output, 0, displayPos.rows - cursorPos.rows);
        }
        self.output.write(result);
        _cursorTo(self.output, cursorPos.cols);
        const totalLine = `${self.getPrompt()}${self.line}${suffix}`;
        const newPos = _getDisplayPosHelper(totalLine);
        const rows = newPos.rows - cursorPos.rows -
          (newPos.cols === 0 ? 1 : 0);
        if (rows > 0) _moveCursor(self.output, 0, -rows);
      });
    }

    function showPreview(showCompletion = true) {
      if (!usePreview) return;
      if (inputPreview !== null || !self.isCompletionEnabled) return;

      const line = self.line.trim();
      if (line === "") return;

      // Show completion preview (inline dim suffix)
      if (showCompletion) {
        showCompletionPreview(self.line);
      }

      // Don't show eval preview in multiline mode
      if (self[kBufferedCommandSymbol]) return;

      // Evaluate for input preview
      let previewLine = line;
      if (
        completionPreview !== null && isCursorAtInputEnd() &&
        escaped !== self.line
      ) {
        previewLine += completionPreview;
      }

      // Use vm.Script with a timeout for preview evaluation to avoid
      // hanging on infinite loops (e.g. `while(true){}`).
      let result;
      try {
        let previewCode = previewLine + "\n";
        // Apply object literal wrapping for preview too
        if (
          isObjectLiteral(previewCode) &&
          !/;\s*$/.test(previewCode.trim())
        ) {
          previewCode = `(${previewCode.trim()})\n`;
        }
        const script = new vm.Script(previewCode, { filename: "repl" });
        if (self.useGlobal) {
          result = script.runInThisContext({
            displayErrors: false,
            timeout: 500,
          });
        } else {
          result = script.runInContext(self.context, {
            displayErrors: false,
            timeout: 500,
          });
        }
      } catch {
        // Timeout, syntax error, or runtime error - no preview
        return;
      }

      if (result === undefined && self.ignoreUndefined) return;

      let inspected = inspect(result, {
        colors: false,
        showProxy: true,
        breakLength: Infinity,
        compact: true,
        maxArrayLength: 10,
        depth: 1,
      });
      if (inspected === line) return;

      // Truncate at newline
      const nlIdx = inspected.search(/[\r\n\v]/);
      if (nlIdx !== -1) inspected = inspected.slice(0, nlIdx);

      // Limit length
      const maxCols = Math.min(
        (self as any).columns || 80,
        250,
      );
      if (inspected.length > maxCols) {
        inspected = inspected.slice(0, maxCols - 4) + "...";
      }

      inputPreview = inspected;

      const preview = self.useColors
        ? `\x1b[90m${inspected}\x1b[39m`
        : `// ${inspected}`;

      const { cursorPos, displayPos } = getPreviewPos();
      const rows = displayPos.rows - cursorPos.rows;
      if (rows > 0) _moveCursor(self.output, 0, rows);
      self.output.write(`\n${preview}`);
      _cursorTo(self.output, cursorPos.cols);
      _moveCursor(self.output, 0, -rows - 1);
    }

    function clearPreview(key: any) {
      if (!usePreview) return;

      // Clear input preview
      if (inputPreview !== null) {
        const { displayPos, cursorPos } = getPreviewPos();
        const rows = displayPos.rows - cursorPos.rows + 1;
        _moveCursor(self.output, 0, rows);
        _clearLine(self.output);
        _moveCursor(self.output, 0, -rows);
        inputPreview = null;
      }

      // Clear completion preview
      if (completionPreview !== null) {
        const move = self.line.length !== self.cursor;
        let pos: any;
        let rows = 0;
        if (move) {
          pos = getPreviewPos();
          _cursorTo(self.output, pos.displayPos.cols);
          rows = pos.displayPos.rows - pos.cursorPos.rows;
          _moveCursor(self.output, 0, rows);
        }
        const totalLine = `${self.getPrompt()}${self.line}${completionPreview}`;
        const newPos = _getDisplayPosHelper(totalLine);
        if (
          newPos.rows === 0 ||
          (move && pos.displayPos.rows === newPos.rows)
        ) {
          _clearLine(self.output, 1);
        } else {
          self.output.write("\x1b[0J");
        }
        if (move) {
          _cursorTo(self.output, pos.cursorPos.cols);
          _moveCursor(self.output, 0, -rows);
        }

        // Auto-accept completion on Enter
        if (key && !key.ctrl && !key.shift) {
          if (key.name === "escape") {
            if (escaped === null && key.meta) {
              escaped = self.line;
            }
          } else if (
            (key.name === "return" || key.name === "enter") &&
            !key.meta &&
            escaped !== self.line &&
            isCursorAtInputEnd()
          ) {
            self._insertString(completionPreview);
          }
        }

        completionPreview = null;
      }

      if (escaped !== self.line) {
        escaped = null;
      }
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
          e !== null && e !== undefined &&
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

        if (e !== null && e !== undefined) {
          self._handleError((e as Recoverable).err || e);
        }

        // Clear buffer if no SyntaxErrors
        self.clearBufferedCommand();
        sawCtrlD = false;

        // If we got any output - print it (if no error)
        if (
          (e === null || e === undefined) &&
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
        clearPreview(key);
        ttyWrite(d, key);
        const showCompletion = key.name !== "escape";
        showPreview(showCompletion);
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
            .replace(/^REPL\d+:\d+\r?\n/, "")
            .replace(/^repl:\d+\r?\n/, "");

          // Deno's V8 doesn't include source context in SyntaxError stacks
          // like Node.js does. Add it if we have the source code attached.
          if ((e as any)._replSourceCode) {
            const srcLine = (e as any)._replSourceCode;
            // Try to determine caret position from the error message.
            // Node.js uses V8's Message.GetStartColumn() which we don't have.
            let col = 0;
            const tokenMatch = e.message.match(
              /Unexpected token '(.+?)'/,
            );
            if (tokenMatch) {
              const idx = srcLine.indexOf(tokenMatch[1]);
              if (idx !== -1) col = idx;
            }
            const caret = " ".repeat(col) + "^";
            errStack = `${srcLine}\n${caret}\n\n${errStack}`;
          }
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
