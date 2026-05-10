// Copyright 2018-2026 the Deno authors. MIT license.

(function () {
const { core, primordials } = globalThis.__bootstrap;
const { Console } = core.loadExtScript(
  "ext:deno_node/internal/console/constructor.mjs",
);

// By the time this IIFE runs (lazy), bootstrap has completed and
// `globalThis.console` is the Deno-Web `Console` instance.
const console = globalThis.console;

const { ObjectAssign } = primordials;

ObjectAssign(console, { Console });

return {
  default: console,
  Console,
  assert: console.assert,
  clear: console.clear,
  count: console.count,
  countReset: console.countReset,
  debug: console.debug,
  dir: console.dir,
  dirxml: console.dirxml,
  error: console.error,
  group: console.group,
  groupCollapsed: console.groupCollapsed,
  groupEnd: console.groupEnd,
  info: console.info,
  log: console.log,
  profile: console.profile,
  profileEnd: console.profileEnd,
  table: console.table,
  time: console.time,
  timeEnd: console.timeEnd,
  timeLog: console.timeLog,
  timeStamp: console.timeStamp,
  trace: console.trace,
  warn: console.warn,
  // deno-lint-ignore no-explicit-any
  indentLevel: (console as any)?.indentLevel,
};
})();
