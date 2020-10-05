import { existsSync } from "../../fs/mod.ts";
import { fromFileUrl } from "../path.ts";

type openFlags =
  | "a"
  | "ax"
  | "a+"
  | "ax+"
  | "as"
  | "as+"
  | "r"
  | "r+"
  | "rs+"
  | "w"
  | "wx"
  | "w+"
  | "wx+";

type openCallback = (err: Error | undefined, fd: number) => void;

function convertFlagAndModeToOptions(
  flag?: openFlags,
  mode?: number,
): Deno.OpenOptions {
  if (!flag) return {};
  if (flag === "a" || flag === "as") {
    return {
      append: true,
      create: true,
      mode,
    };
  }
  if (flag === "ax") {
    return {
      append: true,
      mode,
    };
  }
  if (flag === "a+" || flag === "as+") {
    return {
      append: true,
      create: true,
      read: true,
      mode,
    };
  }
  if (flag === "ax+") {
    return {
      append: true,
      read: true,
      mode,
    };
  }
  if (flag === "r") {
    return {
      read: true,
      mode,
    };
  }
  if (flag === "r+") {
    return {
      read: true,
      write: true,
      mode,
    };
  }
  if (flag === "w") {
    return {
      write: true,
      create: true,
      truncate: true,
      mode,
    };
  }
  if (flag === "wx") {
    return {
      write: true,
      truncate: true,
      mode,
    };
  }
  if (flag === "w+") {
    return {
      write: true,
      create: true,
      truncate: true,
      read: true,
      mode,
    };
  }
  if (flag === "wx+") {
    return {
      write: true,
      truncate: true,
      read: true,
      mode,
    };
  }
  throw new Error("flag doesn't exits");
}

export function open(path: string | URL, callback: openCallback): void;
export function open(
  path: string | URL,
  flags: openFlags,
  callback: openCallback,
): void;
export function open(
  path: string | URL,
  flags: openFlags,
  mode: number,
  callback: openCallback,
): void;
export function open(
  path: string | URL,
  flagsOrCallback: openCallback | openFlags,
  callbackOrMode?: openCallback | number,
  maybeCallback?: openCallback,
) {
  const flags = typeof flagsOrCallback === "string"
    ? flagsOrCallback
    : undefined;
  const callback = typeof flagsOrCallback === "function"
    ? flagsOrCallback
    : typeof callbackOrMode === "function"
    ? callbackOrMode
    : maybeCallback;
  const mode = typeof callbackOrMode === "number" ? callbackOrMode : undefined;
  path = path instanceof URL ? fromFileUrl(path) : path;

  if (!callback) throw new Error("No callback function supplied");

  if (["ax", "ax+", "wx", "wx+"].includes(flags || "") && existsSync(path)) {
    const err = new Error(`EEXIST: file already exists, open '${path}'`);
    callback(err, 0);
  } else {
    if (flags === "as" || flags === "as+") {
      try {
        const res = openSync(path, flags, mode);
        callback(undefined, res);
      } catch (error) {
        callback(error, error);
      }
      return;
    }
    Deno.open(
      path,
      ((flags || mode) && convertFlagAndModeToOptions(flags, mode)) ||
        undefined,
    )
      .then((file) => callback(undefined, file.rid))
      .catch((err) => callback(err, err));
  }
}

export function openSync(path: string | URL): number;
export function openSync(path: string | URL, flags?: openFlags): number;
export function openSync(path: string | URL, mode?: number): number;
export function openSync(
  path: string | URL,
  flags?: openFlags,
  mode?: number,
): number;
export function openSync(
  path: string | URL,
  flagsOrMode?: openFlags | number,
  maybeMode?: number,
) {
  const flags = typeof flagsOrMode === "string" ? flagsOrMode : undefined;
  const mode = typeof flagsOrMode === "number" ? flagsOrMode : maybeMode;
  path = path instanceof URL ? fromFileUrl(path) : path;

  if (["ax", "ax+", "wx", "wx+"].includes(flags || "") && existsSync(path)) {
    throw new Error(`EEXIST: file already exists, open '${path}'`);
  }

  return Deno.openSync(
    path,
    ((flags || mode) && convertFlagAndModeToOptions(flags, mode)) || undefined,
  ).rid;
}
