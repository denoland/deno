// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { open, openSync } from "./files.ts";
import { readAll, readAllSync } from "./buffer.ts";
import { pathFromURL } from "./util.ts";

export function readFileSync(path: string | URL): Uint8Array {
  if (path instanceof URL) {
    path = pathFromURL(path);
  }
  const file = openSync(path);
  const contents = readAllSync(file);
  file.close();
  return contents;
}

export async function readFile(path: string | URL): Promise<Uint8Array> {
  if (path instanceof URL) {
    path = pathFromURL(path);
  }
  const file = await open(path);
  const contents = await readAll(file);
  file.close();
  return contents;
}
