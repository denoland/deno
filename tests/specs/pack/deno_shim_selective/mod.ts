// File that uses Deno APIs
export function getCwd(): string {
  return Deno.cwd();
}
