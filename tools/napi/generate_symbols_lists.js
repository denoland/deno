#!/usr/bin/env -S deno run --unstable --allow-read --allow-write
// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

import exports from "../../cli/napi_sym/symbol_exports.json" assert {
  type: "json",
};

for await (const os of ["linux", "macos", "windows"]) {
  let def = os === "windows" ? "LIBRARY\nEXPORTS\n" : "";
  const prefix = os === "windows" ? "  " : os === "macos" ? "_" : "";
  for (const symbol of exports.symbols) {
    def += `${prefix}${symbol}\n`;
  }

  const defUrl = new URL(
    `../../cli/generated_symbol_exports_list_${os}.def`,
    import.meta.url,
  );
  await Deno.writeTextFile(defUrl.pathname, def, { create: true });
}
