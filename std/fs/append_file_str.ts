// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.

/**
 * Append the string to file synchronously.
 *
 * @param filename File for appending
 * @param content The content append to file
 * @returns void
 */
export function appendFileStrSync(filename: string, content: string): void {
  const encoder = new TextEncoder();
  Deno.writeFileSync(filename, encoder.encode(content), { append: true });
}

/**
 * Append the string to file.
 *
 * @param filename File for appending
 * @param content The content append to file
 * @returns Promise<void>
 */
export async function appendFileStr(
  filename: string,
  content: string
): Promise<void> {
  const encoder = new TextEncoder();
  await Deno.writeFile(filename, encoder.encode(content), { append: true });
}
