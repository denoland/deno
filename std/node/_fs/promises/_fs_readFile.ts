// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import type {
  FileOptionsArgument,
  BinaryOptionsArgument,
  TextOptionsArgument,
} from "../_fs_common.ts";
import { readFile as readFileCallback } from "../_fs_readFile.ts";

export function readFile(
  path: string | URL,
  options: TextOptionsArgument,
): Promise<string>;
export function readFile(
  path: string | URL,
  options?: BinaryOptionsArgument,
): Promise<Uint8Array>;
export function readFile(
  path: string | URL,
  options?: FileOptionsArgument,
): Promise<string | Uint8Array> {
  return new Promise((resolve, reject) => {
    readFileCallback(path, options, (err, data): void => {
      if (err) return reject(err);
      if (data == null) {
        return reject(new Error("Invalid state: data missing, but no error"));
      }
      resolve(data);
    });
  });
}
