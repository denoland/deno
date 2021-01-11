// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.
import {
  CallbackWithError,
  getOpenOptions,
  isFileOptions,
  WriteFileOptions,
} from "./_fs_common.ts";
import { Encodings, notImplemented } from "../_utils.ts";
import { fromFileUrl } from "../path.ts";

/**
 * TODO: Also accept 'data' parameter as a Node polyfill Buffer type once these
 * are implemented. See https://github.com/denoland/deno/issues/3403
 */
export function appendFile(
  pathOrRid: string | number | URL,
  data: string | Uint8Array,
  optionsOrCallback: Encodings | WriteFileOptions | CallbackWithError,
  callback?: CallbackWithError,
): void {
  pathOrRid = pathOrRid instanceof URL ? fromFileUrl(pathOrRid) : pathOrRid;
  const callbackFn: CallbackWithError | undefined =
    optionsOrCallback instanceof Function ? optionsOrCallback : callback;
  const options: Encodings | WriteFileOptions | undefined =
    optionsOrCallback instanceof Function ? undefined : optionsOrCallback;
  if (!callbackFn) {
    throw new Error("No callback function supplied");
  }

  validateEncoding(options);
  let rid = -1;
  const buffer: Uint8Array = data instanceof Uint8Array
    ? data
    : new TextEncoder().encode(data);
  new Promise((resolve, reject) => {
    if (typeof pathOrRid === "number") {
      rid = pathOrRid;
      Deno.write(rid, buffer).then(resolve).catch(reject);
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
      Deno.open(pathOrRid as string, getOpenOptions(flag))
        .then(({ rid: openedFileRid }) => {
          rid = openedFileRid;
          return Deno.write(openedFileRid, buffer);
        })
        .then(resolve)
        .catch(reject);
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
 * TODO: Also accept 'data' parameter as a Node polyfill Buffer type once these
 * are implemented. See https://github.com/denoland/deno/issues/3403
 */
export function appendFileSync(
  pathOrRid: string | number | URL,
  data: string | Uint8Array,
  options?: Encodings | WriteFileOptions,
): void {
  let rid = -1;

  validateEncoding(options);
  pathOrRid = pathOrRid instanceof URL ? fromFileUrl(pathOrRid) : pathOrRid;

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

    const buffer: Uint8Array = data instanceof Uint8Array
      ? data
      : new TextEncoder().encode(data);

    Deno.writeSync(rid, buffer);
  } finally {
    closeRidIfNecessary(typeof pathOrRid === "string", rid);
  }
}

function validateEncoding(
  encodingOption: Encodings | WriteFileOptions | undefined,
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
