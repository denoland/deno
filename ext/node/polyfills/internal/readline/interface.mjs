// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.
// Copyright Joyent, Inc. and other Node contributors.
//
// Permission is hereby granted, free of charge, to any person obtaining a
// copy of this software and associated documentation files (the
// "Software"), to deal in the Software without restriction, including
// without limitation the rights to use, copy, modify, merge, publish,
// distribute, sublicense, and/or sell copies of the Software, and to permit
// persons to whom the Software is furnished to do so, subject to the
// following conditions:
//
// The above copyright notice and this permission notice shall be included
// in all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS
// OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
// MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN
// NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM,
// DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR
// OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE
// USE OR OTHER DEALINGS IN THE SOFTWARE.

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file prefer-primordials
// deno-lint-ignore-file camelcase no-inner-declarations no-this-alias

import {
  ERR_INVALID_ARG_VALUE,
  ERR_USE_AFTER_CLOSE,
} from "ext:deno_node/internal/errors.ts";
import {
  validateAbortSignal,
  validateArray,
  validateString,
  validateUint32,
} from "ext:deno_node/internal/validators.mjs";
import {
  //   inspect,
  getStringWidth,
  stripVTControlCharacters,
} from "ext:deno_node/internal/util/inspect.mjs";
import EventEmitter from "node:events";
import { emitKeypressEvents } from "ext:deno_node/internal/readline/emitKeypressEvents.mjs";
import {
  charLengthAt,
  charLengthLeft,
  commonPrefix,
  kSubstringSearch,
} from "ext:deno_node/internal/readline/utils.mjs";
import {
  clearScreenDown,
  cursorTo,
  moveCursor,
} from "ext:deno_node/internal/readline/callbacks.mjs";
import { Readable } from "ext:deno_node/_stream.mjs";
import process from "node:process";

import { StringDecoder } from "node:string_decoder";
import {
  kAddHistory,
  kDecoder,
  kDeleteLeft,
  kDeleteLineLeft,
  kDeleteLineRight,
  kDeleteRight,
  kDeleteWordLeft,
  kDeleteWordRight,
  kGetDisplayPos,
  kHistoryNext,
  kHistoryPrev,
  kInsertString,
  kLine,
  kLine_buffer,
  kMoveCursor,
  kNormalWrite,
  kOldPrompt,
  kOnLine,
  kPreviousKey,
  kPrompt,
  kQuestionCallback,
  kRefreshLine,
  kSawKeyPress,
  kSawReturnAt,
  kSetRawMode,
  kTabComplete,
  kTabCompleter,
  kTtyWrite,
  kWordLeft,
  kWordRight,
  kWriteToOutput,
} from "ext:deno_node/internal/readline/symbols.mjs";

const kHistorySize = 30;
const kMincrlfDelay = 100;
// \r\n, \n, or \r followed by something other than \n
const lineEnding = /\r?\n|\r(?!\n)/;

const kLineObjectStream = Symbol("line object stream");
export const kQuestionCancel = Symbol("kQuestionCancel");
export const kQuestion = Symbol("kQuestion");

// GNU readline library - keyseq-timeout is 500ms (default)
const ESCAPE_CODE_TIMEOUT = 500;

export {
  kAddHistory,
  kDecoder,
  kDeleteLeft,
  kDeleteLineLeft,
  kDeleteLineRight,
  kDeleteRight,
  kDeleteWordLeft,
  kDeleteWordRight,
  kGetDisplayPos,
  kHistoryNext,
  kHistoryPrev,
  kInsertString,
  kLine,
  kLine_buffer,
  kMoveCursor,
  kNormalWrite,
  kOldPrompt,
  kOnLine,
  kPreviousKey,
  kPrompt,
  kQuestionCallback,
  kRefreshLine,
  kSawKeyPress,
  kSawReturnAt,
  kSetRawMode,
  kTabComplete,
  kTabCompleter,
  kTtyWrite,
  kWordLeft,
  kWordRight,
  kWriteToOutput,
};

export function InterfaceConstructor(input, output, completer, terminal) {
  this[kSawReturnAt] = 0;
  // TODO(BridgeAR): Document this property. The name is not ideal, so we
  // might want to expose an alias and document that instead.
  this.isCompletionEnabled = true;
  this[kSawKeyPress] = false;
  this[kPreviousKey] = null;
  this.escapeCodeTimeout = ESCAPE_CODE_TIMEOUT;
  this.tabSize = 8;
  Function.prototype.call(EventEmitter, this);

  let history;
  let historySize;
  let removeHistoryDuplicates = false;
  let crlfDelay;
  let prompt = "> ";
  let signal;

  if (input?.input) {
    // An options object was given
    output = input.output;
    completer = input.completer;
    terminal = input.terminal;
    history = input.history;
    historySize = input.historySize;
    signal = input.signal;
    if (input.tabSize !== undefined) {
      validateUint32(input.tabSize, "tabSize", true);
      this.tabSize = input.tabSize;
    }
    removeHistoryDuplicates = input.removeHistoryDuplicates;
    if (input.prompt !== undefined) {
      prompt = input.prompt;
    }
    if (input.escapeCodeTimeout !== undefined) {
      if (Number.isFinite(input.escapeCodeTimeout)) {
        this.escapeCodeTimeout = input.escapeCodeTimeout;
      } else {
        throw new ERR_INVALID_ARG_VALUE(
          "input.escapeCodeTimeout",
          this.escapeCodeTimeout,
        );
      }
    }

    if (signal) {
      validateAbortSignal(signal, "options.signal");
    }

    crlfDelay = input.crlfDelay;
    input = input.input;
  }

  if (completer !== undefined && typeof completer !== "function") {
    throw new ERR_INVALID_ARG_VALUE("completer", completer);
  }

  if (history === undefined) {
    history = [];
  } else {
    validateArray(history, "history");
  }

  if (historySize === undefined) {
    historySize = kHistorySize;
  }

  if (
    typeof historySize !== "number" ||
    Number.isNaN(historySize) ||
    historySize < 0
  ) {
    throw new ERR_INVALID_ARG_VALUE.RangeError("historySize", historySize);
  }

  // Backwards compat; check the isTTY prop of the output stream
  //  when `terminal` was not specified
  if (terminal === undefined && !(output === null || output === undefined)) {
    terminal = !!output.isTTY;
  }

  const self = this;

  this.line = "";
  this[kSubstringSearch] = null;
  this.output = output;
  this.input = input;
  this.history = history;
  this.historySize = historySize;
  this.removeHistoryDuplicates = !!removeHistoryDuplicates;
  this.crlfDelay = crlfDelay
    ? Math.max(kMincrlfDelay, crlfDelay)
    : kMincrlfDelay;
  this.completer = completer;

  this.setPrompt(prompt);

  this.terminal = !!terminal;

  function onerror(err) {
    self.emit("error", err);
  }

  function ondata(data) {
    self[kNormalWrite](data);
  }

  function onend() {
    if (
      typeof self[kLine_buffer] === "string" &&
      self[kLine_buffer].length > 0
    ) {
      self.emit("line", self[kLine_buffer]);
    }
    self.close();
  }

  function ontermend() {
    if (typeof self.line === "string" && self.line.length > 0) {
      self.emit("line", self.line);
    }
    self.close();
  }

  function onkeypress(s, key) {
    self[kTtyWrite](s, key);
    if (key && key.sequence) {
      // If the key.sequence is half of a surrogate pair
      // (>= 0xd800 and <= 0xdfff), refresh the line so
      // the character is displayed appropriately.
      const ch = key.sequence.codePointAt(0);
      if (ch >= 0xd800 && ch <= 0xdfff) self[kRefreshLine]();
    }
  }

  function onresize() {
    self[kRefreshLine]();
  }

  this[kLineObjectStream] = undefined;

  input.on("error", onerror);

  if (!this.terminal) {
    function onSelfCloseWithoutTerminal() {
      input.removeListener("data", ondata);
      input.removeListener("error", onerror);
      input.removeListener("end", onend);
    }

    input.on("data", ondata);
    input.on("end", onend);
    self.once("close", onSelfCloseWithoutTerminal);
    this[kDecoder] = new StringDecoder("utf8");
  } else {
    function onSelfCloseWithTerminal() {
      input.removeListener("keypress", onkeypress);
      input.removeListener("error", onerror);
      input.removeListener("end", ontermend);
      if (output !== null && output !== undefined) {
        output.removeListener("resize", onresize);
      }
    }

    emitKeypressEvents(input, this);

    // `input` usually refers to stdin
    input.on("keypress", onkeypress);
    input.on("end", ontermend);

    this[kSetRawMode](true);
    this.terminal = true;

    // Cursor position on the line.
    this.cursor = 0;

    this.historyIndex = -1;

    if (output !== null && output !== undefined) {
      output.on("resize", onresize);
    }

    self.once("close", onSelfCloseWithTerminal);
  }

  if (signal) {
    const onAborted = () => self.close();
    if (signal.aborted) {
      process.nextTick(onAborted);
    } else {
      signal.addEventListener("abort", onAborted, { once: true });
      self.once("close", () => signal.removeEventListener("abort", onAborted));
    }
  }

  // Current line
  this.line = "";

  input.resume();
}

Object.setPrototypeOf(InterfaceConstructor.prototype, EventEmitter.prototype);
Object.setPrototypeOf(InterfaceConstructor, EventEmitter);

export class Interface extends InterfaceConstructor {
  // eslint-disable-next-line no-useless-constructor
  constructor(input, output, completer, terminal) {
    super(input, output, completer, terminal);
  }
  get columns() {
    if (this.output && this.output.columns) return this.output.columns;
    return Infinity;
  }

  /**
   * Sets the prompt written to the output.
   * @param {string} prompt
   * @returns {void}
   */
  setPrompt(prompt) {
    this[kPrompt] = prompt;
  }

  /**
   * Returns the current prompt used by `rl.prompt()`.
   * @returns {string}
   */
  getPrompt() {
    return this[kPrompt];
  }

  [kSetRawMode](mode) {
    const wasInRawMode = this.input.isRaw;

    if (typeof this.input.setRawMode === "function") {
      this.input.setRawMode(mode);
    }

    return wasInRawMode;
  }

  /**
   * Writes the configured `prompt` to a new line in `output`.
   * @param {boolean} [preserveCursor]
   * @returns {void}
   */
  prompt(preserveCursor) {
    if (this.paused) this.resume();
    if (this.terminal && process.env.TERM !== "dumb") {
      if (!preserveCursor) this.cursor = 0;
      this[kRefreshLine]();
    } else {
      this[kWriteToOutput](this[kPrompt]);
    }
  }

  [kQuestion](query, cb) {
    if (this.closed) {
      throw new ERR_USE_AFTER_CLOSE("readline");
    }
    if (this[kQuestionCallback]) {
      this.prompt();
    } else {
      this[kOldPrompt] = this[kPrompt];
      this.setPrompt(query);
      this[kQuestionCallback] = cb;
      this.prompt();
    }
  }

  [kOnLine](line) {
    if (this[kQuestionCallback]) {
      const cb = this[kQuestionCallback];
      this[kQuestionCallback] = null;
      this.setPrompt(this[kOldPrompt]);
      cb(line);
    } else {
      this.emit("line", line);
    }
  }

  [kQuestionCancel]() {
    if (this[kQuestionCallback]) {
      this[kQuestionCallback] = null;
      this.setPrompt(this[kOldPrompt]);
      this.clearLine();
    }
  }

  [kWriteToOutput](stringToWrite) {
    validateString(stringToWrite, "stringToWrite");

    if (this.output !== null && this.output !== undefined) {
      this.output.write(stringToWrite);
    }
  }

  [kAddHistory]() {
    if (this.line.length === 0) return "";

    // If the history is disabled then return the line
    if (this.historySize === 0) return this.line;

    // If the trimmed line is empty then return the line
    if (this.line.trim().length === 0) return this.line;

    if (this.history.length === 0 || this.history[0] !== this.line) {
      if (this.removeHistoryDuplicates) {
        // Remove older history line if identical to new one
        const dupIndex = this.history.indexOf(this.line);
        if (dupIndex !== -1) this.history.splice(dupIndex, 1);
      }

      this.history.unshift(this.line);

      // Only store so many
      if (this.history.length > this.historySize) {
        this.history.pop();
      }
    }

    this.historyIndex = -1;

    // The listener could change the history object, possibly
    // to remove the last added entry if it is sensitive and should
    // not be persisted in the history, like a password
    const line = this.history[0];

    // Emit history event to notify listeners of update
    this.emit("history", this.history);

    return line;
  }

  [kRefreshLine]() {
    // line length
    const line = this[kPrompt] + this.line;
    const dispPos = this[kGetDisplayPos](line);
    const lineCols = dispPos.cols;
    const lineRows = dispPos.rows;

    // cursor position
    const cursorPos = this.getCursorPos();

    // First move to the bottom of the current line, based on cursor pos
    const prevRows = this.prevRows || 0;
    if (prevRows > 0) {
      moveCursor(this.output, 0, -prevRows);
    }

    // Cursor to left edge.
    cursorTo(this.output, 0);
    // erase data
    clearScreenDown(this.output);

    // Write the prompt and the current buffer content.
    this[kWriteToOutput](line);

    // Force terminal to allocate a new line
    if (lineCols === 0) {
      this[kWriteToOutput](" ");
    }

    // Move cursor to original position.
    cursorTo(this.output, cursorPos.cols);

    const diff = lineRows - cursorPos.rows;
    if (diff > 0) {
      moveCursor(this.output, 0, -diff);
    }

    this.prevRows = cursorPos.rows;
  }

  /**
   * Closes the `readline.Interface` instance.
   * @returns {void}
   */
  close() {
    if (this.closed) return;
    this.pause();
    if (this.terminal) {
      this[kSetRawMode](false);
    }
    this.closed = true;
    this.emit("close");
  }

  /**
   * Pauses the `input` stream.
   * @returns {void | Interface}
   */
  pause() {
    if (this.paused) return;
    this.input.pause();
    this.paused = true;
    this.emit("pause");
    return this;
  }

  /**
   * Resumes the `input` stream if paused.
   * @returns {void | Interface}
   */
  resume() {
    if (!this.paused) return;
    this.input.resume();
    this.paused = false;
    this.emit("resume");
    return this;
  }

  /**
   * Writes either `data` or a `key` sequence identified by
   * `key` to the `output`.
   * @param {string} d
   * @param {{
   *   ctrl?: boolean;
   *   meta?: boolean;
   *   shift?: boolean;
   *   name?: string;
   *   }} [key]
   * @returns {void}
   */
  write(d, key) {
    if (this.paused) this.resume();
    if (this.terminal) {
      this[kTtyWrite](d, key);
    } else {
      this[kNormalWrite](d);
    }
  }

  [kNormalWrite](b) {
    if (b === undefined) {
      return;
    }
    let string = this[kDecoder].write(b);
    if (
      this[kSawReturnAt] &&
      Date.now() - this[kSawReturnAt] <= this.crlfDelay
    ) {
      string = string.replace(/^\n/, "");
      this[kSawReturnAt] = 0;
    }

    // Run test() on the new string chunk, not on the entire line buffer.
    const newPartContainsEnding = lineEnding.test(string);

    if (this[kLine_buffer]) {
      string = this[kLine_buffer] + string;
      this[kLine_buffer] = null;
    }
    if (newPartContainsEnding) {
      this[kSawReturnAt] = string.endsWith("\r") ? Date.now() : 0;

      // Got one or more newlines; process into "line" events
      const lines = string.split(lineEnding);
      // Either '' or (conceivably) the unfinished portion of the next line
      string = lines.pop();
      this[kLine_buffer] = string;
      for (let n = 0; n < lines.length; n++) this[kOnLine](lines[n]);
    } else if (string) {
      // No newlines this time, save what we have for next time
      this[kLine_buffer] = string;
    }
  }

  [kInsertString](c) {
    if (this.cursor < this.line.length) {
      const beg = this.line.slice(0, this.cursor);
      const end = this.line.slice(
        this.cursor,
        this.line.length,
      );
      this.line = beg + c + end;
      this.cursor += c.length;
      this[kRefreshLine]();
    } else {
      this.line += c;
      this.cursor += c.length;

      if (this.getCursorPos().cols === 0) {
        this[kRefreshLine]();
      } else {
        this[kWriteToOutput](c);
      }
    }
  }

  async [kTabComplete](lastKeypressWasTab) {
    this.pause();
    const string = this.line.slice(0, this.cursor);
    let value;
    try {
      value = await this.completer(string);
    } catch (err) {
      // TODO(bartlomieju): inspect is not ported yet
      // this[kWriteToOutput](`Tab completion error: ${inspect(err)}`);
      this[kWriteToOutput](`Tab completion error: ${err}`);
      return;
    } finally {
      this.resume();
    }
    this[kTabCompleter](lastKeypressWasTab, value);
  }

  [kTabCompleter](lastKeypressWasTab, { 0: completions, 1: completeOn }) {
    // Result and the text that was completed.

    if (!completions || completions.length === 0) {
      return;
    }

    // If there is a common prefix to all matches, then apply that portion.
    const prefix = commonPrefix(
      completions.filter((e) => e !== ""),
    );
    if (
      prefix.startsWith(completeOn) &&
      prefix.length > completeOn.length
    ) {
      this[kInsertString](prefix.slice(completeOn.length));
      return;
    } else if (!completeOn.startsWith(prefix)) {
      this.line = this.line.slice(0, this.cursor - completeOn.length) +
        prefix +
        this.line.slice(this.cursor, this.line.length);
      this.cursor = this.cursor - completeOn.length + prefix.length;
      this._refreshLine();
      return;
    }

    if (!lastKeypressWasTab) {
      return;
    }

    // Apply/show completions.
    const completionsWidth = completions.map(
      (e) => getStringWidth(e),
    );
    const width = Math.max.apply(completionsWidth) + 2; // 2 space padding
    let maxColumns = Math.floor(this.columns / width) || 1;
    if (maxColumns === Infinity) {
      maxColumns = 1;
    }
    let output = "\r\n";
    let lineIndex = 0;
    let whitespace = 0;
    for (let i = 0; i < completions.length; i++) {
      const completion = completions[i];
      if (completion === "" || lineIndex === maxColumns) {
        output += "\r\n";
        lineIndex = 0;
        whitespace = 0;
      } else {
        output += " ".repeat(whitespace);
      }
      if (completion !== "") {
        output += completion;
        whitespace = width - completionsWidth[i];
        lineIndex++;
      } else {
        output += "\r\n";
      }
    }
    if (lineIndex !== 0) {
      output += "\r\n\r\n";
    }
    this[kWriteToOutput](output);
    this[kRefreshLine]();
  }

  [kWordLeft]() {
    if (this.cursor > 0) {
      // Reverse the string and match a word near beginning
      // to avoid quadratic time complexity
      const leading = this.line.slice(0, this.cursor);
      const reversed = Array.from(leading).reverse().join("");
      const match = reversed.match(/^\s*(?:[^\w\s]+|\w+)?/);
      this[kMoveCursor](-match[0].length);
    }
  }

  [kWordRight]() {
    if (this.cursor < this.line.length) {
      const trailing = this.line.slice(this.cursor);
      const match = trailing.match(/^(?:\s+|[^\w\s]+|\w+)\s*/);
      this[kMoveCursor](match[0].length);
    }
  }

  [kDeleteLeft]() {
    if (this.cursor > 0 && this.line.length > 0) {
      // The number of UTF-16 units comprising the character to the left
      const charSize = charLengthLeft(this.line, this.cursor);
      this.line = this.line.slice(0, this.cursor - charSize) +
        this.line.slice(this.cursor, this.line.length);

      this.cursor -= charSize;
      this[kRefreshLine]();
    }
  }

  [kDeleteRight]() {
    if (this.cursor < this.line.length) {
      // The number of UTF-16 units comprising the character to the left
      const charSize = charLengthAt(this.line, this.cursor);
      this.line = this.line.slice(0, this.cursor) +
        this.line.slice(
          this.cursor + charSize,
          this.line.length,
        );
      this[kRefreshLine]();
    }
  }

  [kDeleteWordLeft]() {
    if (this.cursor > 0) {
      // Reverse the string and match a word near beginning
      // to avoid quadratic time complexity
      let leading = this.line.slice(0, this.cursor);
      const reversed = Array.from(leading).reverse().join("");
      const match = reversed.match(/^\s*(?:[^\w\s]+|\w+)?/);
      leading = leading.slice(
        0,
        leading.length - match[0].length,
      );
      this.line = leading +
        this.line.slice(this.cursor, this.line.length);
      this.cursor = leading.length;
      this[kRefreshLine]();
    }
  }

  [kDeleteWordRight]() {
    if (this.cursor < this.line.length) {
      const trailing = this.line.slice(this.cursor);
      const match = trailing.match(/^(?:\s+|\W+|\w+)\s*/);
      this.line = this.line.slice(0, this.cursor) +
        trailing.slice(match[0].length);
      this[kRefreshLine]();
    }
  }

  [kDeleteLineLeft]() {
    this.line = this.line.slice(this.cursor);
    this.cursor = 0;
    this[kRefreshLine]();
  }

  [kDeleteLineRight]() {
    this.line = this.line.slice(0, this.cursor);
    this[kRefreshLine]();
  }

  clearLine() {
    this[kMoveCursor](+Infinity);
    this[kWriteToOutput]("\r\n");
    this.line = "";
    this.cursor = 0;
    this.prevRows = 0;
  }

  [kLine]() {
    const line = this[kAddHistory]();
    this.clearLine();
    this[kOnLine](line);
  }

  // TODO(BridgeAR): Add underscores to the search part and a red background in
  // case no match is found. This should only be the visual part and not the
  // actual line content!
  // TODO(BridgeAR): In case the substring based search is active and the end is
  // reached, show a comment how to search the history as before. E.g., using
  // <ctrl> + N. Only show this after two/three UPs or DOWNs, not on the first
  // one.
  [kHistoryNext]() {
    if (this.historyIndex >= 0) {
      const search = this[kSubstringSearch] || "";
      let index = this.historyIndex - 1;
      while (
        index >= 0 &&
        (!this.history[index].startsWith(search) ||
          this.line === this.history[index])
      ) {
        index--;
      }
      if (index === -1) {
        this.line = search;
      } else {
        this.line = this.history[index];
      }
      this.historyIndex = index;
      this.cursor = this.line.length; // Set cursor to end of line.
      this[kRefreshLine]();
    }
  }

  [kHistoryPrev]() {
    if (this.historyIndex < this.history.length && this.history.length) {
      const search = this[kSubstringSearch] || "";
      let index = this.historyIndex + 1;
      while (
        index < this.history.length &&
        (!this.history[index].startsWith(search) ||
          this.line === this.history[index])
      ) {
        index++;
      }
      if (index === this.history.length) {
        this.line = search;
      } else {
        this.line = this.history[index];
      }
      this.historyIndex = index;
      this.cursor = this.line.length; // Set cursor to end of line.
      this[kRefreshLine]();
    }
  }

  // Returns the last character's display position of the given string
  [kGetDisplayPos](str) {
    let offset = 0;
    const col = this.columns;
    let rows = 0;
    str = stripVTControlCharacters(str);
    for (const char of str[Symbol.iterator]()) {
      if (char === "\n") {
        // Rows must be incremented by 1 even if offset = 0 or col = +Infinity.
        rows += Math.ceil(offset / col) || 1;
        offset = 0;
        continue;
      }
      // Tabs must be aligned by an offset of the tab size.
      if (char === "\t") {
        offset += this.tabSize - (offset % this.tabSize);
        continue;
      }
      const width = getStringWidth(char);
      if (width === 0 || width === 1) {
        offset += width;
      } else {
        // width === 2
        if ((offset + 1) % col === 0) {
          offset++;
        }
        offset += 2;
      }
    }
    const cols = offset % col;
    rows += (offset - cols) / col;
    return { cols, rows };
  }

  /**
   * Returns the real position of the cursor in relation
   * to the input prompt + string.
   * @returns {{
   *   rows: number;
   *   cols: number;
   *   }}
   */
  getCursorPos() {
    const strBeforeCursor = this[kPrompt] +
      this.line.slice(0, this.cursor);
    return this[kGetDisplayPos](strBeforeCursor);
  }

  // This function moves cursor dx places to the right
  // (-dx for left) and refreshes the line if it is needed.
  [kMoveCursor](dx) {
    if (dx === 0) {
      return;
    }
    const oldPos = this.getCursorPos();
    this.cursor += dx;

    // Bounds check
    if (this.cursor < 0) {
      this.cursor = 0;
    } else if (this.cursor > this.line.length) {
      this.cursor = this.line.length;
    }

    const newPos = this.getCursorPos();

    // Check if cursor stayed on the line.
    if (oldPos.rows === newPos.rows) {
      const diffWidth = newPos.cols - oldPos.cols;
      moveCursor(this.output, diffWidth, 0);
    } else {
      this[kRefreshLine]();
    }
  }

  // Handle a write from the tty
  [kTtyWrite](s, key) {
    const previousKey = this[kPreviousKey];
    key = key || {};
    this[kPreviousKey] = key;

    // Activate or deactivate substring search.
    if (
      (key.name === "up" || key.name === "down") &&
      !key.ctrl &&
      !key.meta &&
      !key.shift
    ) {
      if (this[kSubstringSearch] === null) {
        this[kSubstringSearch] = this.line.slice(
          0,
          this.cursor,
        );
      }
    } else if (this[kSubstringSearch] !== null) {
      this[kSubstringSearch] = null;
      // Reset the index in case there's no match.
      if (this.history.length === this.historyIndex) {
        this.historyIndex = -1;
      }
    }

    // Ignore escape key, fixes
    // https://github.com/nodejs/node-v0.x-archive/issues/2876.
    if (key.name === "escape") return;

    if (key.ctrl && key.shift) {
      /* Control and shift pressed */
      switch (key.name) {
        // TODO(BridgeAR): The transmitted escape sequence is `\b` and that is
        // identical to <ctrl>-h. It should have a unique escape sequence.
        case "backspace":
          this[kDeleteLineLeft]();
          break;

        case "delete":
          this[kDeleteLineRight]();
          break;
      }
    } else if (key.ctrl) {
      /* Control key pressed */

      switch (key.name) {
        case "c":
          if (this.listenerCount("SIGINT") > 0) {
            this.emit("SIGINT");
          } else {
            // This readline instance is finished
            this.close();
          }
          break;

        case "h": // delete left
          this[kDeleteLeft]();
          break;

        case "d": // delete right or EOF
          if (this.cursor === 0 && this.line.length === 0) {
            // This readline instance is finished
            this.close();
          } else if (this.cursor < this.line.length) {
            this[kDeleteRight]();
          }
          break;

        case "u": // Delete from current to start of line
          this[kDeleteLineLeft]();
          break;

        case "k": // Delete from current to end of line
          this[kDeleteLineRight]();
          break;

        case "a": // Go to the start of the line
          this[kMoveCursor](-Infinity);
          break;

        case "e": // Go to the end of the line
          this[kMoveCursor](+Infinity);
          break;

        case "b": // back one character
          this[kMoveCursor](-charLengthLeft(this.line, this.cursor));
          break;

        case "f": // Forward one character
          this[kMoveCursor](+charLengthAt(this.line, this.cursor));
          break;

        case "l": // Clear the whole screen
          cursorTo(this.output, 0, 0);
          clearScreenDown(this.output);
          this[kRefreshLine]();
          break;

        case "n": // next history item
          this[kHistoryNext]();
          break;

        case "p": // Previous history item
          this[kHistoryPrev]();
          break;

        case "z":
          if (process.platform === "win32") break;
          if (this.listenerCount("SIGTSTP") > 0) {
            this.emit("SIGTSTP");
          } else {
            process.once("SIGCONT", () => {
              // Don't raise events if stream has already been abandoned.
              if (!this.paused) {
                // Stream must be paused and resumed after SIGCONT to catch
                // SIGINT, SIGTSTP, and EOF.
                this.pause();
                this.emit("SIGCONT");
              }
              // Explicitly re-enable "raw mode" and move the cursor to
              // the correct position.
              // See https://github.com/joyent/node/issues/3295.
              this[kSetRawMode](true);
              this[kRefreshLine]();
            });
            this[kSetRawMode](false);
            process.kill(process.pid, "SIGTSTP");
          }
          break;

        case "w": // Delete backwards to a word boundary
        // TODO(BridgeAR): The transmitted escape sequence is `\b` and that is
        // identical to <ctrl>-h. It should have a unique escape sequence.
        // Falls through
        case "backspace":
          this[kDeleteWordLeft]();
          break;

        case "delete": // Delete forward to a word boundary
          this[kDeleteWordRight]();
          break;

        case "left":
          this[kWordLeft]();
          break;

        case "right":
          this[kWordRight]();
          break;
      }
    } else if (key.meta) {
      /* Meta key pressed */

      switch (key.name) {
        case "b": // backward word
          this[kWordLeft]();
          break;

        case "f": // forward word
          this[kWordRight]();
          break;

        case "d": // delete forward word
        case "delete":
          this[kDeleteWordRight]();
          break;

        case "backspace": // Delete backwards to a word boundary
          this[kDeleteWordLeft]();
          break;
      }
    } else {
      /* No modifier keys used */

      // \r bookkeeping is only relevant if a \n comes right after.
      if (this[kSawReturnAt] && key.name !== "enter") this[kSawReturnAt] = 0;

      switch (key.name) {
        case "return": // Carriage return, i.e. \r
          this[kSawReturnAt] = Date.now();
          this[kLine]();
          break;

        case "enter":
          // When key interval > crlfDelay
          if (
            this[kSawReturnAt] === 0 ||
            Date.now() - this[kSawReturnAt] > this.crlfDelay
          ) {
            this[kLine]();
          }
          this[kSawReturnAt] = 0;
          break;

        case "backspace":
          this[kDeleteLeft]();
          break;

        case "delete":
          this[kDeleteRight]();
          break;

        case "left":
          // Obtain the code point to the left
          this[kMoveCursor](-charLengthLeft(this.line, this.cursor));
          break;

        case "right":
          this[kMoveCursor](+charLengthAt(this.line, this.cursor));
          break;

        case "home":
          this[kMoveCursor](-Infinity);
          break;

        case "end":
          this[kMoveCursor](+Infinity);
          break;

        case "up":
          this[kHistoryPrev]();
          break;

        case "down":
          this[kHistoryNext]();
          break;

        case "tab":
          // If tab completion enabled, do that...
          if (
            typeof this.completer === "function" &&
            this.isCompletionEnabled
          ) {
            const lastKeypressWasTab = previousKey &&
              previousKey.name === "tab";
            this[kTabComplete](lastKeypressWasTab);
            break;
          }
        // falls through
        default:
          if (typeof s === "string" && s) {
            const lines = s.split(/\r\n|\n|\r/);
            for (let i = 0, len = lines.length; i < len; i++) {
              if (i > 0) {
                this[kLine]();
              }
              this[kInsertString](lines[i]);
            }
          }
      }
    }
  }

  /**
   * Creates an `AsyncIterator` object that iterates through
   * each line in the input stream as a string.
   * @typedef {{
   *   [Symbol.asyncIterator]: () => InterfaceAsyncIterator,
   *   next: () => Promise<string>
   * }} InterfaceAsyncIterator
   * @returns {InterfaceAsyncIterator}
   */
  [Symbol.asyncIterator]() {
    if (this[kLineObjectStream] === undefined) {
      const readable = new Readable({
        objectMode: true,
        read: () => {
          this.resume();
        },
        destroy: (err, cb) => {
          this.off("line", lineListener);
          this.off("close", closeListener);
          this.close();
          cb(err);
        },
      });
      const lineListener = (input) => {
        if (!readable.push(input)) {
          // TODO(rexagod): drain to resume flow
          this.pause();
        }
      };
      const closeListener = () => {
        readable.push(null);
      };
      const errorListener = (err) => {
        readable.destroy(err);
      };
      this.on("error", errorListener);
      this.on("line", lineListener);
      this.on("close", closeListener);
      this[kLineObjectStream] = readable;
    }

    return this[kLineObjectStream][Symbol.asyncIterator]();
  }
}
