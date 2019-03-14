// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import * as path from "./path/mod.ts";

/** Reads a JSON file and then parses it into an object */
export async function readJson(filePath: string): Promise<any> {
  filePath = path.resolve(filePath);
  const decoder = new TextDecoder("utf-8");

  const content = decoder.decode(await Deno.readFile(filePath));

  try {
    return JSON.parse(content);
  } catch (err) {
    err.message = `${filePath}: ${err.message}`;
    throw err;
  }
}

/** Reads a JSON file and then parses it into an object */
export function readJsonSync(filePath: string): any {
  filePath = path.resolve(filePath);
  const decoder = new TextDecoder("utf-8");

  const content = decoder.decode(Deno.readFileSync(filePath));

  try {
    return JSON.parse(content);
  } catch (err) {
    err.message = `${filePath}: ${err.message}`;
    throw err;
  }
}
