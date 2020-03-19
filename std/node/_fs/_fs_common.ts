// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

export type CallbackWithError = (err?: Error) => void;

export interface FileOptions {
  encoding?: string;
  mode?: number;
  flag?: string;
}

export function isFileOptions(
  fileOptions: string | FileOptions | undefined
): fileOptions is FileOptions {
  if (!fileOptions) return false;

  return (
    (fileOptions as FileOptions).encoding != undefined ||
    (fileOptions as FileOptions).flag != undefined ||
    (fileOptions as FileOptions).mode != undefined
  );
}
