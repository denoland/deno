// Copyright 2018-2026 the Deno authors. MIT license.

(function () {
"use strict";

const { primordials } = __bootstrap;
const {
  String,
  StringPrototypeReplaceAll,
  StringPrototypeRepeat,
} = primordials;

function tapEscape(input) {
  // Node's TAP encoder replaces control characters with their two-char
  // backslash escape, then escapes literal backslashes, then escapes `#`.
  // Order matters: doubling backslashes last ensures the escapes introduced
  // by the previous step also get their backslashes doubled.
  let s = String(input);
  s = StringPrototypeReplaceAll(s, "\b", "\\b");
  s = StringPrototypeReplaceAll(s, "\f", "\\f");
  s = StringPrototypeReplaceAll(s, "\v", "\\v");
  s = StringPrototypeReplaceAll(s, "\n", "\\n");
  s = StringPrototypeReplaceAll(s, "\r", "\\r");
  s = StringPrototypeReplaceAll(s, "\t", "\\t");
  s = StringPrototypeReplaceAll(s, "\\", "\\\\");
  s = StringPrototypeReplaceAll(s, "#", "\\#");
  return s;
}

function tapIndent(depth) {
  return StringPrototypeRepeat("    ", depth || 0);
}

return {
  tapEscape,
  tapIndent,
};
})();
