// A non-analyzable specifier, so the import cannot be statically resolved by
// the graph and exercises the runtime dynamic-import path.
function nonAnalyzablePath() {
  return "./settings.toml";
}

// Statically-analyzable dynamic import via file extension.
const { default: yaml } = await import("./config.yaml");
console.log("yaml.name:", (yaml as Record<string, unknown>).name);

// Statically-analyzable dynamic import via import attribute.
const { default: json5 } = await import("./data.json5", {
  with: { type: "json5" },
});
console.log("json5.a:", (json5 as { a: number }).a);

// Non-analyzable dynamic import.
const { default: toml } = await import(nonAnalyzablePath(), {
  with: { type: "toml" },
});
console.log(
  "toml.server.port:",
  (toml as { server: { port: number } }).server.port,
);
