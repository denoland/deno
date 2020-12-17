// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import { ERR_INVALID_ARG_TYPE, ERR_OUT_OF_RANGE } from "./_errors.ts";
import nodeProcess, { Env } from "./process.ts";
import { notImplemented } from "./_utils.ts";
import { ReadableOptions } from "./_stream/readable.ts";
import nodeOs from "./os.ts";

let OSRelease: string[];

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

interface TTY_ENVS {
  eterm: number;
  cons25: number;
  console: number;
  cygwin: number;
  dtterm: number;
  gnome: number;
  hurd: number;
  jfbterm: number;
  konsole: number;
  kterm: number;
  mlterm: number;
  mosh: number;
  putty: number;
  st: number;
  // https =//github.com/da-x/rxvt-unicode/tree/v9.22-with-24bit-color
  "rxvt-unicode-24bit": number;
  // https =//gist.github.com/XVilka/8346728#gistcomment-2823421
  terminator: number;
}

const TERM_ENVS: TTY_ENVS = {
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
function warnOnDeactivatedColors(env: Env) {
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
    // TODO(jopemachine): correct below statement

    // process.emitWarning(
    //   `The '${name}' env is ignored due to the 'FORCE_COLOR' env being set.`,
    //   'Warning'
    // );
    warned = true;
  }
}

// The `getColorDepth` API got inspired by multiple sources such as
// https://github.com/chalk/supports-color,
// https://github.com/isaacs/color-support.
export function getColorDepth(env = nodeProcess.env) {
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

  if (nodeProcess.platform === "win32") {
    // Lazy load for startup performance.
    if (OSRelease === undefined) {
      const { release } = nodeOs;
      OSRelease = release().split(".");
    }
    // Windows 10 build 10586 is the first Windows release that supports 256
    // colors. Windows 10 build 14931 is the first release that supports
    // 16m/TrueColor.
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
      "TRAVIS" in env || "CIRCLECI" in env || "APPVEYOR" in env ||
      "GITLAB_CI" in env || env.CI_NAME === "codeship"
    ) {
      return COLORS_256;
    }
    return COLORS_2;
  }

  if ("TEAMCITY_VERSION" in env) {
    return /^(9\.(0*[1-9]\d*)\.|\d{2,}\.)/.test(env.TEAMCITY_VERSION)
      ? COLORS_16
      : COLORS_2;
  }

  switch (env.TERM_PROGRAM) {
    case "iTerm.app":
      if (
        !env.TERM_PROGRAM_VERSION ||
        /^[0-2]\./.test(env.TERM_PROGRAM_VERSION)
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
    if (/^xterm-256/.test(env.TERM)) {
      return COLORS_256;
    }

    const termEnv = env.TERM.toLowerCase() as keyof TTY_ENVS;

    if (TERM_ENVS[termEnv]) {
      return TERM_ENVS[termEnv];
    }
    for (const term of TERM_ENVS_REG_EXP) {
      if (term.test(termEnv)) {
        return COLORS_16;
      }
    }
  }
  // Move 16 color COLORTERM below 16m and 256
  if (env.COLORTERM) {
    return COLORS_16;
  }
  return COLORS_2;
}

export function hasColors(count: number, env: Env) {
  if (
    env === undefined &&
    (count === undefined || (typeof count === "object" && count !== null))
  ) {
    env = count;
    count = 16;
  } else {
    if (typeof count !== "number") {
      throw new ERR_INVALID_ARG_TYPE("count", "number", count);
    }
    if (count < 2) {
      throw new ERR_OUT_OF_RANGE("count", ">= 2", count);
    }
  }
  return count <= 2 ** getColorDepth(env);
}

export function isatty(fd: number) {
  // TODO(jopemachine): to be implemented
  notImplemented();
}

export function ReadStream(fd: number, options: ReadableOptions) {
  // TODO(jopemachine): to be implemented
  notImplemented();
}

export function WriteStream(fd: number) {
  // TODO(jopemachine): to be implemented
  notImplemented();
}
