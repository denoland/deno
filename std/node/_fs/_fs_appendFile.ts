// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { FileOptions, isFileOptions, CallbackWithError } from "./_fs_common.ts";
import { notImplemented } from "../_utils.ts";

/**
 * TODO: Also accept 'data' parameter as a Node polyfill Buffer or URL type once these
 * are implemented. See https://github.com/denoland/deno/issues/3403
 */
export function appendFile(
  pathOrRid: string | number,
  data: string,
  optionsOrCallback: string | FileOptions | CallbackWithError,
  callback?: CallbackWithError
): void {
  const callbackFn: CallbackWithError | undefined =
    optionsOrCallback instanceof Function ? optionsOrCallback : callback;
  const options: string | FileOptions | undefined =
    optionsOrCallback instanceof Function ? undefined : optionsOrCallback;
  if (!callbackFn) {
    throw new Error("No callback function supplied");
  }

  validateEncoding(options);

  let rid = -1;
  new Promise(async (resolve, reject) => {
    try {
      if (typeof pathOrRid === "number") {
        rid = pathOrRid;
      } else {
        const mode: number | undefined = isFileOptions(options)
          ? options.mode
          : undefined;
        const flag: string | undefined = isFileOptions(options)
          ? options.flag
          : undefined;

        if (mode) {
          //TODO rework once https://github.com/denoland/deno/issues/4017 completes
          notImplemented("Deno does not yet support setting mode on create");
        }
        const file = await Deno.open(pathOrRid, getOpenOptions(flag));
        rid = file.rid;
      }

      const buffer: Uint8Array = new TextEncoder().encode(data);

      await Deno.write(rid, buffer);
      resolve();
    } catch (err) {
      reject(err);
    }
  })
    .then(() => {
      closeRidIfNecessary(typeof pathOrRid === "string", rid);
      callbackFn();
    })
    .catch((err) => {
      closeRidIfNecessary(typeof pathOrRid === "string", rid);
      callbackFn(err);
    });
}

function closeRidIfNecessary(isPathString: boolean, rid: number): void {
  if (isPathString && rid != -1) {
    //Only close if a path was supplied and a rid allocated
    Deno.close(rid);
  }
}

/**
 * TODO: Also accept 'data' parameter as a Node polyfill Buffer or URL type once these
 * are implemented. See https://github.com/denoland/deno/issues/3403
 */
export function appendFileSync(
  pathOrRid: string | number,
  data: string,
  options?: string | FileOptions
): void {
  let rid = -1;

  validateEncoding(options);

  try {
    if (typeof pathOrRid === "number") {
      rid = pathOrRid;
    } else {
      const mode: number | undefined = isFileOptions(options)
        ? options.mode
        : undefined;
      const flag: string | undefined = isFileOptions(options)
        ? options.flag
        : undefined;

      if (mode) {
        // TODO rework once https://github.com/denoland/deno/issues/4017 completes
        notImplemented("Deno does not yet support setting mode on create");
      }

      const file = Deno.openSync(pathOrRid, getOpenOptions(flag));
      rid = file.rid;
    }

    const buffer: Uint8Array = new TextEncoder().encode(data);

    Deno.writeSync(rid, buffer);
  } finally {
    closeRidIfNecessary(typeof pathOrRid === "string", rid);
  }
}

function validateEncoding(
  encodingOption: string | FileOptions | undefined
): void {
  if (!encodingOption) return;

  if (typeof encodingOption === "string") {
    if (encodingOption !== "utf8") {
      throw new Error("Only 'utf8' encoding is currently supported");
    }
  } else if (encodingOption.encoding && encodingOption.encoding !== "utf8") {
    throw new Error("Only 'utf8' encoding is currently supported");
  }
}

function getOpenOptions(flag: string | undefined): Deno.OpenOptions {
  if (!flag) {
    return { create: true, append: true };
  }

  let openOptions: Deno.OpenOptions;
  switch (flag) {
    case "a": {
      // 'a': Open file for appending. The file is created if it does not exist.
      openOptions = { create: true, append: true };
      break;
    }
    case "ax": {
      // 'ax': Like 'a' but fails if the path exists.
      openOptions = { createNew: true, write: true, append: true };
      break;
    }
    case "a+": {
      // 'a+': Open file for reading and appending. The file is created if it does not exist.
      openOptions = { read: true, create: true, append: true };
      break;
    }
    case "ax+": {
      // 'ax+': Like 'a+' but fails if the path exists.
      openOptions = { read: true, createNew: true, append: true };
      break;
    }
    case "r": {
      // 'r': Open file for reading. An exception occurs if the file does not exist.
      openOptions = { read: true };
      break;
    }
    case "r+": {
      // 'r+': Open file for reading and writing. An exception occurs if the file does not exist.
      openOptions = { read: true, write: true };
      break;
    }
    case "w": {
      // 'w': Open file for writing. The file is created (if it does not exist) or truncated (if it exists).
      openOptions = { create: true, write: true, truncate: true };
      break;
    }
    case "wx": {
      // 'wx': Like 'w' but fails if the path exists.
      openOptions = { createNew: true, write: true };
      break;
    }
    case "w+": {
      // 'w+': Open file for reading and writing. The file is created (if it does not exist) or truncated (if it exists).
      openOptions = { create: true, write: true, truncate: true, read: true };
      break;
    }
    case "wx+": {
      // 'wx+': Like 'w+' but fails if the path exists.
      openOptions = { createNew: true, write: true, read: true };
      break;
    }
    case "as": {
      // 'as': Open file for appending in synchronous mode. The file is created if it does not exist.
      openOptions = { create: true, append: true };
    }
    case "as+": {
      // 'as+': Open file for reading and appending in synchronous mode. The file is created if it does not exist.
      openOptions = { create: true, read: true, append: true };
    }
    case "rs+": {
      // 'rs+': Open file for reading and writing in synchronous mode. Instructs the operating system to bypass the local file system cache.
      openOptions = { create: true, read: true, write: true };
    }
    default: {
      throw new Error(`Unrecognized file system flag: ${flag}`);
    }
  }

  return openOptions;
}
