// Copyright 2018-2026 the Deno authors. MIT license.
// Copyright (c) Sindre Sorhus <sindresorhus@gmail.com> (sindresorhus.com)
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

import { op_node_is_tty, op_set_raw } from "ext:core/ops";
import { core, primordials } from "ext:core/mod.js";
import { ERR_INVALID_FD } from "ext:deno_node/internal/errors.ts";
import { validateInteger } from "ext:deno_node/internal/validators.mjs";
import { TTY } from "ext:deno_node/internal_binding/tty_wrap.ts";
import { Socket } from "node:net";
import { setReadStream } from "ext:deno_node/_process/streams.mjs";
import * as io from "ext:deno_io/12_io.js";
import { getRid } from "ext:deno_node/internal/fs/fd_map.ts";
import { release } from "node:os";
import process from "node:process";

const {
  ArrayPrototypeSome,
  ObjectEntries,
  ObjectPrototypeHasOwnProperty,
  RegExpPrototypeExec,
  SafeMap,
  SafeMapIterator,
  SafeRegExp,
  StringPrototypeSplit,
  StringPrototypeToLowerCase,
} = primordials;
const { internalRidSymbol } = core;

// Helper class to wrap a resource ID as a stream-like object.
// Used for PTY file descriptors (fd > 2) that come from NAPI modules like node-pty.
// Similar to Stdin/Stdout/Stderr classes in io module.
class TTYStream {
  #rid;
  #ref = true;
  #opPromise;

  constructor(rid) {
    this.#rid = rid;
  }

  get [internalRidSymbol]() {
    return this.#rid;
  }

  get rid() {
    return this.#rid;
  }

  async read(p) {
    if (p.length === 0) return 0;
    this.#opPromise = core.read(this.#rid, p);
    if (!this.#ref) {
      core.unrefOpPromise(this.#opPromise);
    }
    const nread = await this.#opPromise;
    return nread === 0 ? null : nread;
  }

  readSync(p) {
    if (p.length === 0) return 0;
    const nread = core.readSync(this.#rid, p);
    return nread === 0 ? null : nread;
  }

  write(p) {
    return core.write(this.#rid, p);
  }

  writeSync(p) {
    return core.writeSync(this.#rid, p);
  }

  close() {
    core.tryClose(this.#rid);
  }

  setRaw(mode, options = { __proto__: null }) {
    const cbreak = !!(options.cbreak ?? false);
    op_set_raw(this.#rid, mode, cbreak);
  }

  isTerminal() {
    return core.isTerminal(this.#rid);
  }

  [io.REF]() {
    this.#ref = true;
    if (this.#opPromise) {
      core.refOpPromise(this.#opPromise);
    }
  }

  [io.UNREF]() {
    this.#ref = false;
    if (this.#opPromise) {
      core.unrefOpPromise(this.#opPromise);
    }
  }
}

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
      `The '${name}' env is ignored due to the 'FORCE_COLOR' env being set.`,
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
function getColorDepth(env = process.env) {
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

// Returns true when the given numeric fd is associated with a TTY and false otherwise.
function isatty(fd) {
  if (typeof fd !== "number" || fd >> 0 !== fd || fd < 0) {
    return false;
  }
  return op_node_is_tty(fd);
}

export class ReadStream extends Socket {
  constructor(fd, options) {
    if (fd >> 0 !== fd || fd < 0) {
      throw new ERR_INVALID_FD(fd);
    }

    let handle;
    // For fd > 2 (PTY from NAPI modules like node-pty), create a TTYStream wrapper
    const isPty = fd > 2;
    if (isPty) {
      // Security: Only allow TTY file descriptors. This prevents access to
      // arbitrary fds (sockets, files, etc.) via tty.ReadStream/WriteStream.
      // PTY devices from node-pty are real TTYs so isatty() returns true.
      if (!op_node_is_tty(fd)) {
        throw new ERR_INVALID_FD(fd);
      }
      // Get the rid from the fd map (will dup and create resource if needed)
      const rid = getRid(fd);
      const stream = new TTYStream(rid);
      handle = new TTY(stream);
    } else {
      // For stdin/stdout/stderr, use the built-in handles
      handle = new TTY(
        fd === 0 ? io.stdin : fd === 1 ? io.stdout : io.stderr,
      );
    }
    super({
      readableHighWaterMark: 0,
      handle,
      manualStart: !isPty, // PTY streams should auto-start reading
      ...options,
    });

    this.isRaw = false;
    this.isTTY = true;
  }

  setRawMode(flag) {
    flag = !!flag;
    this._handle.setRaw(flag);

    this.isRaw = flag;
    return this;
  }
}

setReadStream(ReadStream);

export class WriteStream extends Socket {
  constructor(fd) {
    if (fd >> 0 !== fd || fd < 0) {
      throw new ERR_INVALID_FD(fd);
    }

    let handle;
    if (fd > 2) {
      // Security: Only allow TTY file descriptors. This prevents access to
      // arbitrary fds (sockets, files, etc.) via tty.ReadStream/WriteStream.
      if (!op_node_is_tty(fd)) {
        throw new ERR_INVALID_FD(fd);
      }
      // For fd > 2 (PTY from NAPI modules), create a TTYStream wrapper
      const rid = getRid(fd);
      const stream = new TTYStream(rid);
      handle = new TTY(stream);
    } else {
      // For stdin/stdout/stderr, use the built-in handles
      handle = new TTY(
        fd === 0 ? io.stdin : fd === 1 ? io.stdout : io.stderr,
      );
    }

    super({
      readableHighWaterMark: 0,
      handle,
      manualStart: true,
    });

    try {
      const { columns, rows } = Deno.consoleSize();
      this.columns = columns;
      this.rows = rows;
    } catch {
      // consoleSize can fail if not a real TTY
      this.columns = 80;
      this.rows = 24;
    }
    this.isTTY = true;
  }

  /**
   * @param {number | Record<string, string>} [count]
   * @param {Record<string, string>} [env]
   * @returns {boolean}
   */
  hasColors(count, env) {
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
  }

  /**
   * @param {Record<string, string>} [env]
   * @returns {1 | 4 | 8 | 24}
   */
  getColorDepth(env) {
    return getColorDepth(env);
  }
}

export { isatty };
export default { isatty, WriteStream, ReadStream };
