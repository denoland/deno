// Copyright 2018-2026 the Deno authors. MIT license.
// Copyright Joyent, Inc. and Node.js contributors. All rights reserved. MIT license.

// TODO(petamoriken): enable prefer-primordials for node polyfills
// deno-lint-ignore-file prefer-primordials
// deno-fmt-ignore-file
(function () {
  const kAddHistory = Symbol("_addHistory");
  const kDecoder = Symbol("_decoder");
  const kDeleteLeft = Symbol("_deleteLeft");
  const kDeleteLineLeft = Symbol("_deleteLineLeft");
  const kDeleteLineRight = Symbol("_deleteLineRight");
  const kDeleteRight = Symbol("_deleteRight");
  const kDeleteWordLeft = Symbol("_deleteWordLeft");
  const kDeleteWordRight = Symbol("_deleteWordRight");
  const kGetDisplayPos = Symbol("_getDisplayPos");
  const kHistoryNext = Symbol("_historyNext");
  const kHistoryPrev = Symbol("_historyPrev");
  const kInsertString = Symbol("_insertString");
  const kLine = Symbol("_line");
  const kLine_buffer = Symbol("_line_buffer");
  const kMoveCursor = Symbol("_moveCursor");
  const kNormalWrite = Symbol("_normalWrite");
  const kOldPrompt = Symbol("_oldPrompt");
  const kOnLine = Symbol("_onLine");
  const kPreviousKey = Symbol("_previousKey");
  const kPrompt = Symbol("_prompt");
  const kQuestionCallback = Symbol("_questionCallback");
  const kRefreshLine = Symbol("_refreshLine");
  const kSawKeyPress = Symbol("_sawKeyPress");
  const kSawReturnAt = Symbol("_sawReturnAt");
  const kSetRawMode = Symbol("_setRawMode");
  const kTabComplete = Symbol("_tabComplete");
  const kTabCompleter = Symbol("_tabCompleter");
  const kTtyWrite = Symbol("_ttyWrite");
  const kWordLeft = Symbol("_wordLeft");
  const kWordRight = Symbol("_wordRight");
  const kWriteToOutput = Symbol("_writeToOutput");

  return {
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
})()
