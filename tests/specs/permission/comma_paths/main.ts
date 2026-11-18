// Copyright 2018-2026 the Deno authors. MIT license.

for (
  const path of [
    "file,with,commas.txt",
    "plain.txt",
    "file",
    "with",
    "commas.txt",
  ]
) {
  for (const permission of ["read", "write"]) {
    try {
      if (permission === "read") {
        Deno.readTextFileSync(path);
      } else {
        Deno.writeTextFileSync(path, "updated");
      }
      console.log(`${permission} ${path}: granted`);
    } catch (error) {
      if (!(error instanceof Deno.errors.NotCapable)) throw error;
      console.log(`${permission} ${path}: denied`);
    }
  }
}
