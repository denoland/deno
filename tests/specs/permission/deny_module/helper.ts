export function readSelf(): string {
  // `Deno.statSync` goes through the read-permission check. We stat the
  // helper module itself by URL, which works on every platform.
  const info = Deno.statSync(new URL(import.meta.url));
  return info.isFile ? "file" : "other";
}
