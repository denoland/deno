// This is the workspace member's export. The root config maps "@scope/greet"
// to ./override.ts, so this should be shadowed by the import map.
export function greet(name) {
  return "member: " + name;
}
