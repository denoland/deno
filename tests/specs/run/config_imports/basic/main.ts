// Extension-based imports.
import yaml from "./config.yaml";
import jsonc from "./data.jsonc";
import json5 from "./data.json5";
// Attribute-based import.
import toml from "./settings.toml" with { type: "toml" };
// JSON5/JSONC satisfy `type: "json"`.
import json5AsJson from "./data.json5" with { type: "json" };

const y = yaml as Record<string, unknown>;
const t = toml as { title: string; server: { port: number } };
const jc = jsonc as { a: number; b: number[] };
const j5 = json5 as { a: number; b: string };

console.log("yaml.name:", y.name);
console.log("yaml.nested:", JSON.stringify(y.nested));
console.log("toml.title:", t.title);
console.log("toml.server.port:", t.server.port);
console.log("jsonc.a:", jc.a, "jsonc.b:", JSON.stringify(jc.b));
console.log("json5.a:", j5.a, "json5.b:", j5.b);
console.log(
  "json5AsJson.b:",
  (json5AsJson as { b: string }).b,
);
