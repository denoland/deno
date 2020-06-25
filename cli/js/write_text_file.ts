import { open, openSync } from "./files.ts";
import { writeAll, writeAllSync } from "./buffer.ts";

export function writeTextFileSync(path: string | URL, data: string): void {
  const file = openSync(path, { write: true, create: true, truncate: true });
  const enc = new TextEncoder();
  const contents = enc.encode(data);
  writeAllSync(file, contents);
  file.close();
}

export async function writeTextFile(
  path: string | URL,
  data: string
): Promise<void> {
  const file = await open(path, { write: true, create: true, truncate: true });
  const enc = new TextEncoder();
  const contents = enc.encode(data);
  await writeAll(file, contents);
  file.close();
}
