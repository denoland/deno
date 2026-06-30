// Config imports require an explicit `type` attribute.
import yaml from "./config.yaml" with { type: "yaml" };
import json5 from "./data.json5" with { type: "json5" };
import toml from "./settings.toml" with { type: "toml" };

const y = yaml as Record<string, unknown>;
const t = toml as { title: string; server: { port: number } };
const j5 = json5 as { a: number; b: string };

console.log("yaml.name:", y.name);
console.log("yaml.nested:", JSON.stringify(y.nested));
console.log("toml.title:", t.title);
console.log("toml.server.port:", t.server.port);
console.log("json5.a:", j5.a, "json5.b:", j5.b);
