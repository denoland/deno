// File that uses Deno APIs
export function getEnv(key: string): string | undefined {
  return Deno.env.get(key);
}
