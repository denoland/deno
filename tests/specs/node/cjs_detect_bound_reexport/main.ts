import * as yamlAst from "mock-yaml-ast-parser";

const kind = (yamlAst as { Kind?: unknown }).Kind;
console.log(kind);
