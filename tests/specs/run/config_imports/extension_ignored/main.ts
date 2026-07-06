// The file extension alone does not make this a config import; without an
// explicit `with { type: "yaml" }` attribute it is treated as a plain
// (unsupported) module even when `--unstable-raw-imports` is enabled.
import yaml from "./config.yaml";
console.log(yaml);
