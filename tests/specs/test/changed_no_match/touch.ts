Deno.writeTextFileSync(
  "unrelated.ts",
  Deno.readTextFileSync("unrelated.ts") + "\n// changed\n",
);
