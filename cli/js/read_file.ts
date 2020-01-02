// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { open, openSync } from "./files.ts";
import { readAll, readAllSync } from "./buffer.ts";

/** Read the entire contents of a file synchronously.
 *
 *       const decoder = new TextDecoder("utf-8");
 *       const data = Deno.readFileSync("hello.txt");
 *       console.log(decoder.decode(data));
 */
export function readFileSync(filename: string): Uint8Array {
  const file = openSync(filename);
  const contents = readAllSync(file);
  file.close();
  return contents;
}

/** Read the entire contents of a file.
 *
 *       const decoder = new TextDecoder("utf-8");
 *       const data = await Deno.readFile("hello.txt");
 *       console.log(decoder.decode(data));
 */
export async function readFile(filename: string): Promise<Uint8Array> {
  const file = await open(filename);
  const contents = await readAll(file);
  file.close();
  return contents;
}
