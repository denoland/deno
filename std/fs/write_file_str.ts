// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

/**
 * Write the string to file synchronously.
 *
 * @param filename File to write
 * @param content The content write to file
 * @returns void
 */
export function writeFileStrSync(filename: string, content: string): void {
  const encoder = new TextEncoder();
  Deno.writeFileSync(filename, encoder.encode(content));
}

/**
 * Write the string to file.
 *
 * @param filename File to write
 * @param content The content write to file
 * @returns Promise<void>
 */
export async function writeFileStr(
  filename: string,
  content: string
): Promise<void> {
  const encoder = new TextEncoder();
  await Deno.writeFile(filename, encoder.encode(content));
}
