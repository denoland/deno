const tarPath = [...Deno.readDirSync(".")]
  .map((entry) => entry.name)
  .find((name) => name.endsWith(".tgz"));
if (tarPath == null) {
  throw new Error("pack did not create a tarball");
}

const compressed = await Deno.readFile(tarPath);
const stream = new Blob([compressed])
  .stream()
  .pipeThrough(new DecompressionStream("gzip"));
const reader = stream.getReader();
const chunks: Uint8Array[] = [];
while (true) {
  const { done, value } = await reader.read();
  if (done) break;
  chunks.push(value);
}
const size = chunks.reduce((total, chunk) => total + chunk.length, 0);
const tar = new Uint8Array(size);
let offset = 0;
for (const chunk of chunks) {
  tar.set(chunk, offset);
  offset += chunk.length;
}

const text = new TextDecoder().decode(tar);
const checks = [
  ["regular method", "regular(): number;"],
  ["inherited method", "inherited(): string;"],
  ["class inheritance", "export declare class Resource extends Base {"],
  ["computed symbol", "[customSymbol](): boolean;"],
  ["sync dispose", "[Symbol.dispose](): void;"],
  ["async dispose", "[Symbol.asyncDispose](): Promise<void>;"],
  ["overload", "overloaded(value: string): string;"],
  ["second overload", "overloaded(value: number): number;"],
  ["type import", 'from "./types.js"'],
  ["imported declaration", "package/types.d.ts"],
  ["types metadata", '"types": "./mod.d.ts"'],
];
for (const [name, value] of checks) {
  if (!text.includes(value)) {
    throw new Error(`missing ${name}: ${value}`);
  }
}
if (text.includes("/// <amd-module")) {
  throw new Error("generated declarations expose an internal module path");
}
console.log("computed symbol declarations preserved: true");
