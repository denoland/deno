// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

import { primordials } from "ext:core/mod.js";
import { Console } from "ext:deno_node/internal/console/constructor.mjs";
import { windowOrWorkerGlobalScope } from "ext:runtime/98_global_scope_shared.js";
// Don't rely on global `console` because during bootstrapping, it is pointing
// to native `console` object provided by V8.
const console = windowOrWorkerGlobalScope.console.value;

const { ObjectAssign } = primordials;

ObjectAssign(console, { Console });

export default console;

export { Console };
export const {
  assert,
  clear,
  count,
  countReset,
  debug,
  dir,
  dirxml,
  error,
  group,
  groupCollapsed,
  groupEnd,
  info,
  log,
  profile,
  profileEnd,
  table,
  time,
  timeEnd,
  timeLog,
  timeStamp,
  trace,
  warn,
} = console;
// deno-lint-ignore no-explicit-any
export const indentLevel = (console as any)?.indentLevel;
