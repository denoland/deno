// Copyright 2018-2026 the Deno authors. MIT license.
// Copyright Node.js contributors. All rights reserved. MIT License.

import { op_get_env_no_permission_check } from "ext:core/ops";
import * as io from "ext:deno_io/12_io.js";

let blue = "";
let green = "";
let white = "";
let yellow = "";
let red = "";
let gray = "";
let clear = "";
let reset = "";
let hasColors = false;

function shouldColorize() {
  if (!io.stderr.isTerminal()) {
    return false;
  }

  if (op_get_env_no_permission_check("NODE_DISABLE_COLORS") == "1") {
    return false;
  }

  return !Deno.noColor;
}

function refresh() {
  if (shouldColorize()) {
    blue = "\u001b[34m";
    green = "\u001b[32m";
    white = "\u001b[39m";
    yellow = "\u001b[33m";
    red = "\u001b[31m";
    gray = "\u001b[90m";
    clear = "\u001bc";
    reset = "\u001b[0m";
    hasColors = true;
  } else {
    blue = "";
    green = "";
    white = "";
    yellow = "";
    red = "";
    gray = "";
    clear = "";
    reset = "";
    hasColors = false;
  }
}

refresh();

export {
  blue,
  clear,
  gray,
  green,
  hasColors,
  red,
  refresh,
  reset,
  shouldColorize,
  white,
  yellow,
};
