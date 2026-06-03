try {
  Deno.removeSync("./cowsay_backup", { recursive: true });
} catch {}
try {
  Deno.mkdirSync("./cowsay_backup");
} catch {}
Deno.copyFileSync("./cowsay/main.mjs", "./cowsay_backup/main.mjs");
Deno.copyFileSync("./cowsay/package.json", "./cowsay_backup/package.json");
