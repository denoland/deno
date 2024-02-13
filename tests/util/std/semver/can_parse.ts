// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { SemVer } from "./types.ts";
import { parse } from "./parse.ts";

export function canParse(version: string | SemVer) {
  try {
    parse(version);
    return true;
  } catch (err) {
    if (!(err instanceof TypeError)) {
      throw err;
    }
    return false;
  }
}
