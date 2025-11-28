import $, { Path } from "jsr:@david/dax@^0.42.0";
import * as semver from "@std/semver";
import { UntarStream } from "@std/tar/untar-stream";

const typesNodeVersion = "24.2.0";
$.logStep("Downloading @types/node packument");
const typesNodeInfo = await $.request("https://registry.npmjs.org/@types/node")
  .json();
const typesNodeVersionInfo = typesNodeInfo.versions[typesNodeVersion];

if (typesNodeVersionInfo == null) {
  throw new Error("Could not find types node version info.");
}

if (Object.keys(typesNodeVersionInfo.dependencies).length !== 1) {
  console.error("Dependencies", typesNodeVersionInfo.dependencies);
  throw new Error("Expected only one @types/node dep");
}

const undiciTypesVersionConstraint = semver.parseRange(
  typesNodeVersionInfo.dependencies["undici-types"],
);
if (undiciTypesVersionConstraint == null) {
  console.error("Dependencies", typesNodeVersionInfo.dependencies);
  throw new Error("Failed to find undici-types version constraint.");
}

$.logStep("Downloading undici-types packument");
const undiciTypesInfo = await $.request(
  "https://registry.npmjs.org/undici-types",
).json();
const undiciTypesVersion = semver.format(
  semver.maxSatisfying(
    Object.keys(undiciTypesInfo.versions).map((v) => semver.parse(v)),
    undiciTypesVersionConstraint,
  )!,
);
const undiciTypesVersionInfo = undiciTypesInfo.versions[undiciTypesVersion];
if (undiciTypesVersionInfo == null) {
  throw new Error("Could not find undici types version info.");
}

if (
  undiciTypesVersionInfo.dependencies != null &&
  Object.keys(undiciTypesVersionInfo.dependencies).length !== null
) {
  console.error(undiciTypesVersionInfo.dependencies);
  throw new Error("Expected no deps for undici-types.");
}

const rootDir = $.path(import.meta.url).parentOrThrow().parentOrThrow();
const dtsDir = rootDir.join("cli/tsc/dts/");
const nodeTypesDir = dtsDir.join("node");
const undiciTypesDir = nodeTypesDir.join("undici");

await extractTarball(typesNodeVersionInfo.dist.tarball, nodeTypesDir);
await extractTarball(undiciTypesVersionInfo.dist.tarball, undiciTypesDir);

async function extractTarball(url: string, destination: Path) {
  $.logStep("Downloading", url, "to", destination.toString());
  destination.ensureDirSync();
  destination.emptyDirSync();
  const tarballBlob = await $.request(url).blob();
  for await (
    const entry of tarballBlob.stream()
      .pipeThrough(new DecompressionStream("gzip"))
      .pipeThrough(new UntarStream())
  ) {
    if (
      entry.readable == null ||
      entry.path.endsWith("README.md") ||
      entry.path.endsWith("package.json") ||
      entry.path.includes("ts5.1") ||
      entry.path.includes("ts5.6") ||
      entry.path.includes("ts5.7")
    ) {
      if (entry.readable) {
        await entry.readable.cancel();
      }
      continue;
    }
    const adjustedPath = entry.path
      .replace(/^node\//, "")
      .replace(/^package\//, "");
    const path = destination.join(adjustedPath).resolve();
    path.parentOrThrow().mkdirSync({ recursive: true });
    using file = path.createSync();
    await entry.readable.pipeTo(file.writable);
  }
}
