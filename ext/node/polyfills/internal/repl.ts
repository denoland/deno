// Copyright 2018-2026 the Deno authors. MIT license.
// Copyright Joyent and Node contributors. All rights reserved. MIT license.

// Minimal polyfill of Node's internal/repl module. Tests reach for this
// module via `--expose-internals` to construct a "standalone" REPL with
// env-var overrides applied. The API surface that the suite actually
// exercises is `createInternalRepl(env, opts, cb)` plus the public
// re-exports of node:repl.

// deno-lint-ignore-file no-explicit-any prefer-primordials

import REPL from "node:repl";
import process from "node:process";

const kStandaloneREPL = Symbol.for("nodejs.repl.kStandaloneREPL");

function createRepl(
  env: Record<string, string | undefined>,
  opts: any,
  cb: any,
) {
  if (typeof opts === "function") {
    cb = opts;
    opts = null;
  }
  opts = {
    [kStandaloneREPL]: true,
    ignoreUndefined: false,
    useGlobal: true,
    breakEvalOnSigint: true,
    ...opts,
  };

  if (env && env.NODE_NO_READLINE && Number.parseInt(env.NODE_NO_READLINE)) {
    opts.terminal = false;
  }

  if (env && env.NODE_REPL_MODE) {
    const mode = env.NODE_REPL_MODE.toLowerCase().trim();
    opts.replMode = (REPL as any)[
      mode === "strict" ? "REPL_MODE_STRICT" : "REPL_MODE_SLOPPY"
    ];
  }
  if (opts.replMode === undefined) {
    opts.replMode = (REPL as any).REPL_MODE_SLOPPY;
  }

  const size = Number(env?.NODE_REPL_HISTORY_SIZE);
  if (!Number.isNaN(size) && size > 0) {
    opts.size = size;
  } else {
    opts.size = 1000;
  }

  const term = "terminal" in opts
    ? opts.terminal
    : (process as any).stdout?.isTTY;
  opts.filePath = term ? env?.NODE_REPL_HISTORY : "";

  const repl = (REPL as any).start(opts);

  // Honour both the legacy (filePath, cb) and modern object-form signatures.
  repl.setupHistory({
    filePath: opts.filePath,
    size: opts.size,
    onHistoryFileLoaded: cb,
  });
}

const exported: any = Object.create(REPL);
exported.createInternalRepl = createRepl;

export default exported;
export const createInternalRepl = createRepl;
