// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

import { open, openSync } from "./files.ts";
import { writeAll, writeAllSync } from "./buffer.ts";

export function writeTextFileSync(path: string | URL, data: string): void {
  const file = openSync(path, { write: true, create: true, truncate: true });
  writeAllSync(file, new TextEncoder().encode(data));
  file.close();
}

export async function writeTextFile(
  path: string | URL,
  data: string
): Promise<void> {
  const file = await open(path, { write: true, create: true, truncate: true });
  await writeAll(file, new TextEncoder().encode(data));
  file.close();
}
