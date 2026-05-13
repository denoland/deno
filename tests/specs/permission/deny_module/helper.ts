export function readHostname(): string {
  return Deno.readTextFileSync("/etc/hostname");
}
