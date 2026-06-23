// Mapped by the root config's import map for "@scope/greet". This must win over
// the synthesized workspace member entry, matching how the import map takes
// precedence over workspace members at runtime.
export function greet(name) {
  return "override: " + name;
}
