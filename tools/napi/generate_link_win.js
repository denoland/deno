#!/usr/bin/env -S deno run --unstable --allow-read --allow-write
// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

import exports from "./symbol_exports.json" assert { type: "json" };

let def = "LIBRARY\nEXPORTS\n";
for (const symbol of exports.symbols) {
  def += `  ${symbol}\n`;
}

const defUrl = new URL("../../cli/exports.def", import.meta.url);
await Deno.writeTextFile(defUrl.pathname, def, { create: true });
