Deno.writeTextFileSync(
  "math.ts",
  Deno.readTextFileSync("math.ts") + "\n// changed\n",
);
