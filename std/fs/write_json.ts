// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
// eslint-disable-next-line @typescript-eslint/no-explicit-any
type Replacer = (key: string, value: any) => any;

export interface WriteJsonOptions {
  spaces?: number | string;
  replacer?: Array<number | string> | Replacer;
}

/* Writes an object to a JSON file. */
export async function writeJson(
  filePath: string,
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  object: any,
  options: WriteJsonOptions = {}
): Promise<void> {
  let contentRaw = "";

  try {
    contentRaw = JSON.stringify(
      object,
      options.replacer as string[],
      options.spaces
    );
  } catch (err) {
    err.message = `${filePath}: ${err.message}`;
    throw err;
  }

  await Deno.writeFile(filePath, new TextEncoder().encode(contentRaw));
}

/* Writes an object to a JSON file. */
export function writeJsonSync(
  filePath: string,
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  object: any,
  options: WriteJsonOptions = {}
): void {
  let contentRaw = "";

  try {
    contentRaw = JSON.stringify(
      object,
      options.replacer as string[],
      options.spaces
    );
  } catch (err) {
    err.message = `${filePath}: ${err.message}`;
    throw err;
  }

  Deno.writeFileSync(filePath, new TextEncoder().encode(contentRaw));
}
