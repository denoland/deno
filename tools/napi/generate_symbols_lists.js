#!/usr/bin/env -S deno run --allow-read --allow-write
// Copyright 2018-2025 the Deno authors. MIT license.

import exports from "../../ext/napi/sym/symbol_exports.json" with {
  type: "json",
};

const symbolExportLists = {
  linux: `{ ${exports.symbols.map((s) => `"${s}"`).join("; ")}; };`,
  windows: `LIBRARY\nEXPORTS\n${
    exports.symbols
      .map((symbol) => "  " + symbol)
      .join("\n")
  }`,
  macos: exports.symbols.map((symbol) => "_" + symbol).join("\n"),
};

for await (const [os, def] of Object.entries(symbolExportLists)) {
  const defUrl = new URL(
    `../../ext/napi/generated_symbol_exports_list_${os}.def`,
    import.meta.url,
  );
  await Deno.writeTextFile(defUrl.pathname, def, { create: true });
}
