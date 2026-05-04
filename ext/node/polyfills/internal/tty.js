// Copyright 2018-2026 the Deno authors. MIT license.
// Copyright Joyent, Inc. and Node.js contributors. All rights reserved. MIT license.
// Copyright (c) Sindre Sorhus <sindresorhus@gmail.com> (sindresorhus.com)

// deno-lint-ignore-file no-process-global

import { core, primordials } from "ext:core/mod.js";
const {
  ArrayPrototypeSome,
  FunctionPrototypeCall,
  ObjectEntries,
  ObjectPrototypeHasOwnProperty,
  ObjectPrototypeIsPrototypeOf,
  ObjectSetPrototypeOf,
  RegExpPrototypeExec,
  SafeMap,
  SafeMapIterator,
  SafeRegExp,
  SafeSet,
  SafeSetIterator,
  SetPrototypeAdd,
  SetPrototypeDelete,
  SetPrototypeGetSize,
  StringPrototypeSplit,
  StringPrototypeToLowerCase,
} = primordials;

import {
  ERR_INVALID_FD,
  ERR_TTY_INIT_FAILED,
  errnoException,
} from "ext:deno_node/internal/errors.ts";
const { validateInteger } = core.loadExtScript(
  "ext:deno_node/internal/validators.mjs",
);
import { op_tty_check_fd_permission, TTY } from "ext:core/ops";
import { Socket } from "node:net";
import {
  clearLine,
  clearScreenDown,
  cursorTo,
  moveCursor,
} from "ext:deno_node/internal/readline/callbacks.mjs";
import { release } from "node:os";

// Color depth constants
const COLORS_2 = 1;
const COLORS_16 = 4;
const COLORS_256 = 8;
const COLORS_16M = 24;

// Terminal environments supporting specific color depths
const TERM_ENVS = {
  "eterm": COLORS_16,
  "cons25": COLORS_16,
  "console": COLORS_16,
  "cygwin": COLORS_16,
  "dtterm": COLORS_16,
  "gnome": COLORS_16,
  "hurd": COLORS_16,
  "jfbterm": COLORS_16,
  "konsole": COLORS_16,
  "kterm": COLORS_16,
  "mlterm": COLORS_16,
  "mosh": COLORS_16M,
  "putty": COLORS_16,
  "st": COLORS_16,
  "rxvt-unicode-24bit": COLORS_16M,
  "terminator": COLORS_16M,
  "xterm-kitty": COLORS_16M,
};

// CI environments and their color support
const CI_ENVS_MAP = new SafeMap(ObjectEntries({
  APPVEYOR: COLORS_256,
  BUILDKITE: COLORS_256,
  CIRCLECI: COLORS_16M,
  DRONE: COLORS_256,
  GITEA_ACTIONS: COLORS_16M,
  GITHUB_ACTIONS: COLORS_16M,
  GITLAB_CI: COLORS_256,
  TRAVIS: COLORS_256,
}));

// Regular expressions for terminal types
const TERM_ENVS_REG_EXP = [
  new SafeRegExp("ansi"),
  new SafeRegExp("color"),
  new SafeRegExp("linux"),
  new SafeRegExp("direct"),
  new SafeRegExp("^con[0-9]*x[0-9]"),
  new SafeRegExp("^rxvt"),
  new SafeRegExp("^screen"),
  new SafeRegExp("^xterm"),
  new SafeRegExp("^vt100"),
  new SafeRegExp("^vt220"),
];

let warned = false;
function warnOnDeactivatedColors(env) {
  if (warned) {
    return;
  }
  let name = "";
  if (env.NODE_DISABLE_COLORS !== undefined && env.NODE_DISABLE_COLORS !== "") {
    name = "NODE_DISABLE_COLORS";
  }
  if (env.NO_COLOR !== undefined && env.NO_COLOR !== "") {
    if (name !== "") {
      name += "' and '";
    }
    name += "NO_COLOR";
  }

  if (name !== "") {
    process.emitWarning(
      "The '" + name +
        "' env is ignored due to the 'FORCE_COLOR' env being set.",
      "Warning",
    );
    warned = true;
  }
}

let OSRelease;

/**
 * @param {Record<string, string>} [env]
 * @returns {1 | 4 | 8 | 24}
 */
export function getColorDepth(env = process.env) {
  // Use level 0-3 to support the same levels as `chalk` does.
  if (env.FORCE_COLOR !== undefined) {
    switch (env.FORCE_COLOR) {
      case "":
      case "1":
      case "true":
        warnOnDeactivatedColors(env);
        return COLORS_16;
      case "2":
        warnOnDeactivatedColors(env);
        return COLORS_256;
      case "3":
        warnOnDeactivatedColors(env);
        return COLORS_16M;
      default:
        return COLORS_2;
    }
  }

  if (
    (env.NODE_DISABLE_COLORS !== undefined &&
      env.NODE_DISABLE_COLORS !== "") ||
    (env.NO_COLOR !== undefined && env.NO_COLOR !== "") ||
    env.TERM === "dumb"
  ) {
    return COLORS_2;
  }

  if (process.platform === "win32") {
    if (OSRelease === undefined) {
      OSRelease = StringPrototypeSplit(release(), ".", 3);
    }
    if (+OSRelease[0] >= 10) {
      const build = +OSRelease[2];
      if (build >= 14931) {
        return COLORS_16M;
      }
      if (build >= 10586) {
        return COLORS_256;
      }
    }
    return COLORS_16;
  }

  if (env.TMUX) {
    return COLORS_16M;
  }

  // Azure DevOps
  if (
    ObjectPrototypeHasOwnProperty(env, "TF_BUILD") &&
    ObjectPrototypeHasOwnProperty(env, "AGENT_NAME")
  ) {
    return COLORS_16;
  }

  if (ObjectPrototypeHasOwnProperty(env, "CI")) {
    for (const { 0: envName, 1: colors } of new SafeMapIterator(CI_ENVS_MAP)) {
      if (ObjectPrototypeHasOwnProperty(env, envName)) {
        return colors;
      }
    }
    if (env.CI_NAME === "codeship") {
      return COLORS_256;
    }
    return COLORS_2;
  }

  if (ObjectPrototypeHasOwnProperty(env, "TEAMCITY_VERSION")) {
    return RegExpPrototypeExec(
        new SafeRegExp("^(9\\.(0*[1-9]\\d*)\\.|\\d{2,}\\.)"),
        env.TEAMCITY_VERSION,
      ) !== null
      ? COLORS_16
      : COLORS_2;
  }

  switch (env.TERM_PROGRAM) {
    case "iTerm.app":
      if (
        !env.TERM_PROGRAM_VERSION ||
        RegExpPrototypeExec(
            new SafeRegExp("^[0-2]\\."),
            env.TERM_PROGRAM_VERSION,
          ) !== null
      ) {
        return COLORS_256;
      }
      return COLORS_16M;
    case "HyperTerm":
    case "MacTerm":
      return COLORS_16M;
    case "Apple_Terminal":
      return COLORS_256;
  }

  if (env.COLORTERM === "truecolor" || env.COLORTERM === "24bit") {
    return COLORS_16M;
  }

  if (env.TERM) {
    if (RegExpPrototypeExec(new SafeRegExp("truecolor"), env.TERM) !== null) {
      return COLORS_16M;
    }

    if (RegExpPrototypeExec(new SafeRegExp("^xterm-256"), env.TERM) !== null) {
      return COLORS_256;
    }

    const termEnv = StringPrototypeToLowerCase(env.TERM);

    if (TERM_ENVS[termEnv]) {
      return TERM_ENVS[termEnv];
    }
    if (
      ArrayPrototypeSome(
        TERM_ENVS_REG_EXP,
        (term) => RegExpPrototypeExec(term, termEnv) !== null,
      )
    ) {
      return COLORS_16;
    }
  }

  // Move 16 color COLORTERM below 16m and 256
  if (env.COLORTERM) {
    return COLORS_16;
  }

  return COLORS_2;
}

// Lazy SIGWINCH handling: only register the signal listener when at least one
// WriteStream has a "resize" listener, and unregister when none do. This avoids
// creating a persistent pending op that interferes with event loop exit / TLA
// stall detection. Uses Deno.addSignalListener directly to avoid circular
// dependency with node:process.
const sigwinchStreams = new SafeSet();
let sigwinchRegistered = false;

function onSigwinch() {
  for (const stream of new SafeSetIterator(sigwinchStreams)) {
    stream._refreshSize();
  }
}

function addSigwinchListener(stream) {
  SetPrototypeAdd(sigwinchStreams, stream);
  if (!sigwinchRegistered) {
    sigwinchRegistered = true;
    Deno.addSignalListener("SIGWINCH", onSigwinch);
  }
}

function removeSigwinchListener(stream) {
  SetPrototypeDelete(sigwinchStreams, stream);
  if (SetPrototypeGetSize(sigwinchStreams) === 0 && sigwinchRegistered) {
    sigwinchRegistered = false;
    Deno.removeSignalListener("SIGWINCH", onSigwinch);
  }
}

// WriteStream needs to be callable without `new` to match Node.js behavior.
function WriteStream(fd) {
  if (!ObjectPrototypeIsPrototypeOf(WriteStream.prototype, this)) {
    return new WriteStream(fd);
  }

  if (fd >> 0 !== fd || fd < 0) {
    throw new ERR_INVALID_FD(fd);
  }

  // Non-stdio fds require --allow-all
  op_tty_check_fd_permission(fd);

  const ctx = {};
  const tty = new TTY(fd, ctx);
  if (ctx.code !== undefined) {
    throw new ERR_TTY_INIT_FAILED(ctx);
  }

  FunctionPrototypeCall(Socket, this, {
    readableHighWaterMark: 0,
    handle: tty,
    manualStart: true,
  });

  // Prevents interleaved or dropped stdout/stderr output for terminals.
  // As noted in the following reference, local TTYs tend to be quite fast and
  // this behavior has become expected due historical functionality on OS X,
  // even though it was originally intended to change in v1.0.2 (Libuv 1.2.1).
  // Ref: https://github.com/nodejs/node/pull/1771#issuecomment-119351671
  this._handle.setBlocking(true);

  const winSize = [0, 0];
  const err = tty.getWindowSize(winSize);
  if (!err) {
    this.columns = winSize[0];
    this.rows = winSize[1];
  }
}

ObjectSetPrototypeOf(WriteStream.prototype, Socket.prototype);
ObjectSetPrototypeOf(WriteStream, Socket);

WriteStream.prototype.isTTY = true;

WriteStream.prototype.on = function on(event, listener) {
  FunctionPrototypeCall(Socket.prototype.on, this, event, listener);
  if (event === "resize" && this.listenerCount("resize") === 1) {
    addSigwinchListener(this);
  }
  return this;
};

WriteStream.prototype.addListener = function addListener(event, listener) {
  return this.on(event, listener);
};

WriteStream.prototype.removeListener = function removeListener(
  event,
  listener,
) {
  FunctionPrototypeCall(Socket.prototype.removeListener, this, event, listener);
  if (event === "resize" && this.listenerCount("resize") === 0) {
    removeSigwinchListener(this);
  }
  return this;
};

WriteStream.prototype.off = function off(event, listener) {
  return this.removeListener(event, listener);
};

WriteStream.prototype.removeAllListeners = function removeAllListeners(event) {
  FunctionPrototypeCall(Socket.prototype.removeAllListeners, this, event);
  if (!event || event === "resize") {
    removeSigwinchListener(this);
  }
  return this;
};

WriteStream.prototype._refreshSize = function _refreshSize() {
  const oldCols = this.columns;
  const oldRows = this.rows;
  const winSize = [0, 0];
  const err = this._handle.getWindowSize(winSize);
  if (err) {
    this.emit("error", errnoException(err, "getWindowSize"));
    return;
  }
  const { 0: newCols, 1: newRows } = winSize;
  if (oldCols !== newCols || oldRows !== newRows) {
    this.columns = newCols;
    this.rows = newRows;
    this.emit("resize");
  }
};

WriteStream.prototype.cursorTo = function cursorTo_(x, y, callback) {
  return cursorTo(this, x, y, callback);
};

WriteStream.prototype.moveCursor = function moveCursor_(dx, dy, callback) {
  return moveCursor(this, dx, dy, callback);
};

WriteStream.prototype.clearLine = function clearLine_(dir, callback) {
  return clearLine(this, dir, callback);
};

WriteStream.prototype.clearScreenDown = function clearScreenDown_(callback) {
  return clearScreenDown(this, callback);
};

WriteStream.prototype.getWindowSize = function getWindowSize() {
  return [this.columns, this.rows];
};

/**
 * @param {number | Record<string, string>} [count]
 * @param {Record<string, string>} [env]
 * @returns {boolean}
 */
WriteStream.prototype.hasColors = function hasColors(count, env) {
  if (
    env === undefined &&
    (count === undefined || typeof count === "object" && count !== null)
  ) {
    env = count;
    count = 16;
  } else {
    validateInteger(count, "count", 2);
  }

  const depth = this.getColorDepth(env);
  return count <= 2 ** depth;
};

/**
 * @param {Record<string, string>} [env]
 * @returns {1 | 4 | 8 | 24}
 */
WriteStream.prototype.getColorDepth = function getColorDepth_(env) {
  return getColorDepth(env);
};

export { WriteStream };
export default WriteStream;
