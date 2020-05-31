// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { open, openSync } from "./files.ts";
import { readAll, readAllSync } from "./buffer.ts";

export function readFileSync(path: string | URL): Uint8Array {
  const file = openSync(path);
  const contents = readAllSync(file);
  file.close();
  return contents;
}

export async function readFile(path: string | URL): Promise<Uint8Array> {
  const file = await open(path);
  const contents = await readAll(file);
  file.close();
  return contents;
}
