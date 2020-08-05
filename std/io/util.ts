// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import * as path from "../path/mod.ts";

export function charCode(s: string): number {
  return s.charCodeAt(0);
}

/** Create or open a temporal file at specified directory with prefix and
 *  postfix
 * */
export async function tempFile(
  dir: string,
  opts: {
    prefix?: string;
    postfix?: string;
  } = { prefix: "", postfix: "" },
): Promise<{ file: Deno.File; filepath: string }> {
  const r = Math.floor(Math.random() * 1000000);
  const filepath = path.resolve(
    `${dir}/${opts.prefix || ""}${r}${opts.postfix || ""}`,
  );
  await Deno.mkdir(path.dirname(filepath), { recursive: true });
  const file = await Deno.open(filepath, {
    create: true,
    read: true,
    write: true,
    append: true,
  });
  return { file, filepath };
}
