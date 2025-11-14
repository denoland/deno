// Copyright 2018-2025 the Deno authors. MIT license.
// deno-lint-ignore-file no-console

import tsBlankSpace from "npm:ts-blank-space";

const files = ["./ext/telemetry/telemetry.ts"];

for (const file of files) {
  const content = Deno.readTextFileSync(file);
  console.log(tsBlankSpace(content, (e) => {
    const tokenString = content.slice(e.pos, e.end);

    console.log("Error:", e);
    throw new Error(`Unsupported TypeScript syntax: "${tokenString}"`);
  }));
}
