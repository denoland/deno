// Copyright 2018-2026 the Deno authors. MIT license.
// Copyright Node.js contributors. All rights reserved. MIT License.
(function () {
const { core } = globalThis.__bootstrap;
// Lazy: only used inside shouldColorize(). Defers 12_io.js (which loads
// 06_streams.js) out of the boot snapshot.
let _io;
function io() { return _io || (_io = core.loadExtScript("ext:deno_io/12_io.js")); }

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
  if (!io().stderr.isTerminal()) {
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

// Defer the initial refresh() call: it would invoke isTerminal() on stderr,
// which loads 12_io.js -> 06_streams.js. Instead, expose getter properties
// that run refresh on first access.
let refreshed = false;
function lazyRefresh() {
  if (!refreshed) {
    refreshed = true;
    refresh();
  }
}

const exports_ = {
  refresh() { refreshed = true; refresh(); },
  shouldColorize,
};
const desc = (k, v) => ({
  enumerable: true, configurable: true,
  get() { lazyRefresh(); return v(); },
});
Object.defineProperties(exports_, {
  blue: desc("blue", () => blue),
  clear: desc("clear", () => clear),
  gray: desc("gray", () => gray),
  green: desc("green", () => green),
  hasColors: desc("hasColors", () => hasColors),
  red: desc("red", () => red),
  reset: desc("reset", () => reset),
  white: desc("white", () => white),
  yellow: desc("yellow", () => yellow),
});
return exports_;
})();
