import { writeFileSync, writeFile, WriteFileOptions } from "./write_file.ts";

export function writeTextFileSync(
  path: string | URL,
  data: string,
  options: WriteFileOptions = {}
): void {
  const encoder = new TextEncoder();
  return writeFileSync(path, encoder.encode(data), options);
}

export function writeTextFile(
  path: string | URL,
  data: string,
  options: WriteFileOptions = {}
): Promise<void> {
  const encoder = new TextEncoder();
  return writeFile(path, encoder.encode(data), options);
}
