Deno.writeTextFileSync(
  "./vendor/http_127.0.0.1_4250/@denotest/add/1.0.0/mod.ts",
  `export function add(a: number, b: number): number {
  return a + b + 1; // evil add
}
`,
);
