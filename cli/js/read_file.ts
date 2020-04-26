// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
import { open, openSync } from "./files.ts";
import { readAll, readAllSync } from "./buffer.ts";

export function readFileSync(
  path: string,
  options: { encoding: "utf8" }
): string;
export function readFileSync(path: string, options?: {}): Uint8Array;

export function readFileSync(path: string, options?: {}): Uint8Array | string {
  const file = openSync(path);
  const contents = readAllSync(file, options);
  file.close();
  return contents;
}

export async function readFile(
  path: string,
  options: { encoding: "utf8" }
): Promise<string>;
export async function readFile(path: string, options: {}): Promise<Uint8Array>;

export async function readFile(
  path: string,
  options?: {}
): Promise<Uint8Array | string> {
  const file = await open(path);
  const contents = await readAll(file, options);
  file.close();
  return contents;
}
