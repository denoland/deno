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

const decoder = new TextDecoder();

function readTarString(
  header: Uint8Array,
  start: number,
  length: number,
): string {
  const field = header.subarray(start, start + length);
  const end = field.indexOf(0);
  return decoder.decode(end === -1 ? field : field.subarray(0, end));
}

function parseTarEntries(tar: Uint8Array): Map<string, Uint8Array> {
  const entries = new Map<string, Uint8Array>();
  for (let offset = 0; offset + 512 <= tar.length;) {
    const header = tar.subarray(offset, offset + 512);
    if (header.every((byte) => byte === 0)) break;

    const name = readTarString(header, 0, 100);
    const prefix = readTarString(header, 345, 155);
    const path = prefix === "" ? name : `${prefix}/${name}`;
    const size = Number.parseInt(readTarString(header, 124, 12).trim(), 8);
    if (!Number.isSafeInteger(size) || size < 0) {
      throw new Error(`invalid tar entry size for ${path}`);
    }

    const dataStart = offset + 512;
    const dataEnd = dataStart + size;
    if (dataEnd > tar.length) {
      throw new Error(`truncated tar entry: ${path}`);
    }
    // Only regular files are relevant to the declaration assertions. Keep
    // their bytes separate so a filename or string in another entry cannot
    // satisfy a check accidentally.
    if (header[156] === 0 || header[156] === 0x30) {
      entries.set(path, tar.slice(dataStart, dataEnd));
    }
    offset = dataStart + Math.ceil(size / 512) * 512;
  }
  return entries;
}

const entries = parseTarEntries(tar);
function readText(path: string): string {
  const content = entries.get(path);
  if (content == null) throw new Error(`missing tar entry: ${path}`);
  return decoder.decode(content);
}

const declaration = readText("package/mod.d.ts");
const memberDeclaration = readText("package/member/mod.d.ts");
const typesDeclaration = readText("package/types.d.ts");
const packageJson = JSON.parse(readText("package/package.json"));
const checks = [
  ["root compiler scope", "export declare const rootValue: string;"],
  ["regular method", "regular(): number;"],
  ["inherited method", "inherited(): string;"],
  ["class inheritance", "export declare class Resource extends Base {"],
  ["computed symbol", "[customSymbol](): boolean;"],
  ["sync dispose", "[Symbol.dispose](): void;"],
  ["async dispose", "[Symbol.asyncDispose](): Promise<void>;"],
  ["overload", "overloaded(value: string): string;"],
  ["second overload", "overloaded(value: number): number;"],
  ["type import", 'from "./types.js"'],
];
for (const [name, value] of checks) {
  if (!declaration.includes(value)) {
    throw new Error(`missing ${name}: ${value}`);
  }
}
if (!typesDeclaration.includes("value: string;")) {
  throw new Error("missing imported declaration content");
}
if (
  !memberDeclaration.includes(
    "export declare const looseValue: string | undefined;",
  )
) {
  throw new Error("nested declaration did not use its own compiler scope");
}
if (packageJson.types !== "./mod.d.ts") {
  throw new Error("package metadata does not point to mod.d.ts");
}
if (declaration.includes("/// <amd-module")) {
  throw new Error("generated declarations expose an internal module path");
}
console.log("computed symbol declarations preserved: true");
