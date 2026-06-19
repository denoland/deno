import yaml from "./config.yaml" with { type: "yaml" };
import toml from "./settings.toml" with { type: "toml" };
import json5 from "./data.json5" with { type: "json5" };

console.log("yaml.name:", (yaml as Record<string, unknown>).name);
console.log("toml.port:", (toml as { server: { port: number } }).server.port);
console.log("json5.b:", (json5 as { b: string }).b);
