// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import type { CallbackWithError } from "ext:deno_node/_fs/_fs_common.ts";
import { errors } from "ext:runtime/01_errors.js";
import * as denoFs from "ext:deno_fs/30_fs.js";

function getValidTime(
  time: number | string | Date,
  name: string,
): number | Date {
  if (typeof time === "string") {
    time = Number(time);
  }

  if (
    typeof time === "number" &&
    (Number.isNaN(time) || !Number.isFinite(time))
  ) {
    throw new errors.InvalidData(
      `invalid ${name}, must not be infinity or NaN`,
    );
  }

  return time;
}

export function futimes(
  fd: number,
  atime: number | string | Date,
  mtime: number | string | Date,
  callback: CallbackWithError,
) {
  if (!callback) {
    throw new errors.InvalidData("No callback function supplied");
  }

  atime = getValidTime(atime, "atime");
  mtime = getValidTime(mtime, "mtime");

  denoFs.futime(fd, atime, mtime).then(() => callback(null), callback);
}

export function futimesSync(
  fd: number,
  atime: number | string | Date,
  mtime: number | string | Date,
) {
  atime = getValidTime(atime, "atime");
  mtime = getValidTime(mtime, "mtime");

  denoFs.futimeSync(fd, atime, mtime);
}
