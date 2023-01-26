import { walkSync } from "https://deno.land/std@0.171.0/fs/walk.ts";

for (const entry of walkSync(Deno.args[0])) {
  if (entry.isFile && entry.path.endsWith(".js")) {
    const content = await Deno.readTextFile(entry.path);
    const newContent = content.replace(
      /opAsync\(\s*["']([^"']+)["']\s*,\s*([^)]+)\s*\)/g,
      "ops.$1($2)",
    );
    await Deno.writeTextFile(entry.path, newContent);
  }
}
