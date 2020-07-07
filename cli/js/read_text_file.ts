// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import { open, openSync } from "./files.ts";
import { readAll, readAllSync } from "./buffer.ts";

export function readTextFileSync(path: string | URL): string {
  const file = openSync(path);
  const contents = readAllSync(file);
  file.close();
  return new TextDecoder().decode(contents);
}

export async function readTextFile(path: string | URL): Promise<string> {
  const file = await open(path);
  const contents = await readAll(file);
  file.close();
  return new TextDecoder().decode(contents);
}
