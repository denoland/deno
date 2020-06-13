// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { notImplemented } from "../_utils.ts";
import { fromFileUrl } from "../path.ts";

import {
  WriteFileOptions,
  CallbackWithError,
  isFileOptions,
  getEncoding,
  checkEncoding,
  getOpenOptions,
} from "./_fs_common.ts";

export function writeFile(
  pathOrRid: string | number | URL,
  data: string | Uint8Array,
  optOrCallback: string | CallbackWithError | WriteFileOptions | undefined,
  callback?: CallbackWithError
): void {
  const callbackFn: CallbackWithError | undefined =
    optOrCallback instanceof Function ? optOrCallback : callback;
  const options: string | WriteFileOptions | undefined =
    optOrCallback instanceof Function ? undefined : optOrCallback;

  if (!callbackFn) {
    throw new TypeError("Callback must be a function.");
  }

  pathOrRid = pathOrRid instanceof URL ? fromFileUrl(pathOrRid) : pathOrRid;

  const flag: string | undefined = isFileOptions(options)
    ? options.flag
    : undefined;

  const mode: number | undefined = isFileOptions(options)
    ? options.mode
    : undefined;

  const encoding = checkEncoding(getEncoding(options)) || "utf8";
  const openOptions = getOpenOptions(flag || "w");

  if (typeof data === "string" && encoding === "utf8")
    data = new TextEncoder().encode(data) as Uint8Array;

  const isRid = typeof pathOrRid === "number";
  let file;

  let error: Error | null = null;
  (async (): Promise<void> => {
    try {
      file = isRid
        ? new Deno.File(pathOrRid as number)
        : await Deno.open(pathOrRid as string, openOptions);

      if (!isRid && mode) {
        if (Deno.build.os === "windows") notImplemented(`"mode" on Windows`);
        await Deno.chmod(pathOrRid as string, mode);
      }

      await Deno.writeAll(file, data as Uint8Array);
    } catch (e) {
      error = e;
    } finally {
      // Make sure to close resource
      if (!isRid && file) file.close();
      callbackFn(error);
    }
  })();
}

export function writeFileSync(
  pathOrRid: string | number | URL,
  data: string | Uint8Array,
  options?: string | WriteFileOptions
): void {
  pathOrRid = pathOrRid instanceof URL ? fromFileUrl(pathOrRid) : pathOrRid;

  const flag: string | undefined = isFileOptions(options)
    ? options.flag
    : undefined;

  const mode: number | undefined = isFileOptions(options)
    ? options.mode
    : undefined;

  const encoding = checkEncoding(getEncoding(options)) || "utf8";
  const openOptions = getOpenOptions(flag || "w");

  if (typeof data === "string" && encoding === "utf8")
    data = new TextEncoder().encode(data) as Uint8Array;

  const isRid = typeof pathOrRid === "number";
  let file;

  let error: Error | null = null;
  try {
    file = isRid
      ? new Deno.File(pathOrRid as number)
      : Deno.openSync(pathOrRid as string, openOptions);

    if (!isRid && mode) {
      if (Deno.build.os === "windows") notImplemented(`"mode" on Windows`);
      Deno.chmodSync(pathOrRid as string, mode);
    }

    Deno.writeAllSync(file, data as Uint8Array);
  } catch (e) {
    error = e;
  } finally {
    // Make sure to close resource
    if (!isRid && file) file.close();

    if (error) throw error;
  }
}
