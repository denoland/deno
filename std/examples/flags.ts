// Copyright 2018-2020 the Deno authors. All rights reserved. MIT license.
const { args } = Deno;
import { parse } from "../flags/mod.ts";

if (import.meta.main) {
  console.dir(parse(args));
}
