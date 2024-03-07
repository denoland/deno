// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

// deno-lint-ignore-file prefer-primordials

import { core, primordials } from "ext:core/mod.js";
const {
  Error,
  ArrayPrototypeSome,
  RegExpPrototypeExec,
  StringPrototypeSplit,
  StringPrototypeToLowerCase,
} = primordials;
const {
  isTerminal,
} = core;

import { ERR_INVALID_FD } from "ext:deno_node/internal/errors.ts";
import { LibuvStreamWrap } from "ext:deno_node/internal_binding/stream_wrap.ts";
import { providerType } from "ext:deno_node/internal_binding/async_wrap.ts";
import { Socket } from "node:net";
import { validateInteger } from "ext:deno_node/internal/validators.mjs";
import { setReadStream } from "ext:deno_node/_process/streams.mjs";
import process from "node:process";
import { release } from "node:os";

// Returns true when the given numeric fd is associated with a TTY and false otherwise.
function isatty(fd) {
  if (typeof fd !== "number") {
    return false;
  }
  try {
    /**
     * TODO: Treat `fd` as real file descriptors. Currently, `rid` 0, 1, 2
     * correspond to `fd` 0, 1, 2 (stdin, stdout, stderr). This may change in
     * the future.
     */
    return isTerminal(fd);
  } catch (_) {
    return false;
  }
}

class TTY extends LibuvStreamWrap {
  constructor(handle) {
    super(providerType.TTYWRAP, handle);
  }
}

export class ReadStream extends Socket {
  constructor(fd, options) {
    if (fd >> 0 !== fd || fd < 0) {
      throw new ERR_INVALID_FD(fd);
    }

    // We only support `stdin`.
    if (fd != 0) throw new Error("Only fd 0 is supported.");

    const tty = new TTY(Deno.stdin);
    super({
      readableHighWaterMark: 0,
      handle: tty,
      manualStart: true,
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

    // We only support `stdin`, `stdout` and `stderr`.
    if (fd > 2) throw new Error("Only fd 0, 1 and 2 are supported.");

    const tty = new TTY(
      fd === 0 ? Deno.stdin : fd === 1 ? Deno.stdout : Deno.stderr,
    );

    super({
      readableHighWaterMark: 0,
      handle: tty,
      manualStart: true,
    });

    const { columns, rows } = Deno.consoleSize();
    this.columns = columns;
    this.rows = rows;
  }
}

WriteStream.prototype.isTTY = true;

WriteStream.prototype.getColorDepth = getColorDepth;

WriteStream.prototype.hasColors = hasColors;

// MIT License

// Copyright (c) Sindre Sorhus <sindresorhus@gmail.com> (sindresorhus.com)

// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in
// all copies or substantial portions of the Software.

// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE

let OSRelease;

const COLORS_2 = 1;
const COLORS_16 = 4;
const COLORS_256 = 8;
const COLORS_16M = 24;

// Some entries were taken from `dircolors`
// (https://linux.die.net/man/1/dircolors). The corresponding terminals might
// support more than 16 colors, but this was not tested for.
//
// Copyright (C) 1996-2016 Free Software Foundation, Inc. Copying and
// distribution of this file, with or without modification, are permitted
// provided the copyright notice and this notice are preserved.
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
  // https://github.com/da-x/rxvt-unicode/tree/v9.22-with-24bit-color
  "rxvt-unicode-24bit": COLORS_16M,
  // https://gist.github.com/XVilka/8346728#gistcomment-2823421
  "terminator": COLORS_16M,
};

const TERM_ENVS_REG_EXP = [
  /ansi/,
  /color/,
  /linux/,
  /^con[0-9]*x[0-9]/,
  /^rxvt/,
  /^screen/,
  /^xterm/,
  /^vt100/,
];

let warned = false;
function warnOnDeactivatedColors(env) {
  if (warned) {
    return;
  }
  let name = "";
  if (env.NODE_DISABLE_COLORS !== undefined) {
    name = "NODE_DISABLE_COLORS";
  }
  if (env.NO_COLOR !== undefined) {
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

// The `getColorDepth` API got inspired by multiple sources such as
// https://github.com/chalk/supports-color,
// https://github.com/isaacs/color-support.
function getColorDepth(env = process.env) {
  // Use level 0-3 to support the same levels as `chalk` does. This is done for
  // consistency throughout the ecosystem.
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
    env.NODE_DISABLE_COLORS !== undefined ||
    // See https://no-color.org/
    env.NO_COLOR !== undefined ||
    // The "dumb" special terminal, as defined by terminfo, doesn't support
    // ANSI color control codes.
    // See https://invisible-island.net/ncurses/terminfo.ti.html#toc-_Specials
    env.TERM === "dumb"
  ) {
    return COLORS_2;
  }

  if (process.platform === "win32") {
    if (OSRelease === undefined) {
      OSRelease = StringPrototypeSplit(release(), ".");
    }
    // Windows 10 build 10586 is the first Windows release that supports 256
    // colors. Windows 10 build 14931 is the first release that supports
    // 16M/TrueColor.
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
    return COLORS_256;
  }

  if (env.CI) {
    if (
      [
        "APPVEYOR",
        "BUILDKITE",
        "CIRCLECI",
        "DRONE",
        "GITHUB_ACTIONS",
        "GITLAB_CI",
        "TRAVIS",
      ].some((sign) => sign in env) || env.CI_NAME === "codeship"
    ) {
      return COLORS_256;
    }
    return COLORS_2;
  }

  if ("TEAMCITY_VERSION" in env) {
    return RegExpPrototypeExec(
        /^(9\.(0*[1-9]\d*)\.|\d{2,}\.)/,
        env.TEAMCITY_VERSION,
      ) !== null
      ? COLORS_16
      : COLORS_2;
  }

  switch (env.TERM_PROGRAM) {
    case "iTerm.app":
      if (
        !env.TERM_PROGRAM_VERSION ||
        RegExpPrototypeExec(/^[0-2]\./, env.TERM_PROGRAM_VERSION) !== null
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
    if (RegExpPrototypeExec(/^xterm-256/, env.TERM) !== null) {
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
  // Move 16 color COLORTERM below 16M and 256
  if (env.COLORTERM) {
    return COLORS_16;
  }
  return COLORS_2;
}

function hasColors(count, env) {
  if (
    env === undefined &&
    (count === undefined || (typeof count === "object" && count !== null))
  ) {
    env = count;
    count = 16;
  } else {
    validateInteger(count, "count", 2);
  }

  return count <= 2 ** getColorDepth(env);
}

export { isatty };
export default { isatty, WriteStream, ReadStream };
