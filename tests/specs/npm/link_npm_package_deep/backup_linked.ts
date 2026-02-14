try {
  Deno.removeSync("./linked_backup", { recursive: true });
} catch {}
try {
  Deno.mkdirSync("./linked_backup");
} catch {}
Deno.copyFileSync("./linked/main.mjs", "./linked_backup/main.mjs");
Deno.copyFileSync("./linked/package.json", "./linked_backup/package.json");
