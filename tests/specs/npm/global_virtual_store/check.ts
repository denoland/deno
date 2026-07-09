const packagePath =
  "node_modules/@denotest/esm-package-no-default-export-types";
function normalizePath(path: string) {
  return path.replaceAll("\\", "/");
}

const aRealPath = Deno.realPathSync(`a/${packagePath}`);
const bRealPath = Deno.realPathSync(`b/${packagePath}`);

if (aRealPath !== bRealPath) {
  throw new Error(
    `expected projects to share npm package folder:\n${aRealPath}\n${bRealPath}`,
  );
}

if (!aRealPath.includes("_virtual_store")) {
  throw new Error(
    `expected package real path to use global store: ${aRealPath}`,
  );
}

console.log("shared");

const cLifecycleRealPath = Deno.realPathSync(
  "c/node_modules/@denotest/lifecycle-scripts-counter",
);
const dLifecycleRealPath = Deno.realPathSync(
  "d/node_modules/@denotest/lifecycle-scripts-counter",
);

if (cLifecycleRealPath !== dLifecycleRealPath) {
  throw new Error(
    `expected lifecycle projects to share npm package folder:\n${cLifecycleRealPath}\n${dLifecycleRealPath}`,
  );
}

if (!cLifecycleRealPath.includes("_virtual_store")) {
  throw new Error(
    `expected lifecycle package real path to use global store: ${cLifecycleRealPath}`,
  );
}

const messagePath = `${cLifecycleRealPath}/message.js`;
if (!Deno.statSync(messagePath).isFile) {
  throw new Error(`expected lifecycle script output at ${messagePath}`);
}

const counter = Deno.readTextFileSync("lifecycle-counter.txt");
if (counter !== "run\n") {
  throw new Error(
    `expected lifecycle script to run once, got ${JSON.stringify(counter)}`,
  );
}

console.log("scripts shared");

const eRealPath = Deno.realPathSync(`e/${packagePath}`);
if (eRealPath.includes("_virtual_store")) {
  throw new Error(
    `expected hardlisted package to use project store: ${eRealPath}`,
  );
}
if (!normalizePath(eRealPath).includes("node_modules/.deno")) {
  throw new Error(
    `expected hardlisted package to use local .deno store: ${eRealPath}`,
  );
}

console.log("hardlist fallback");

const hRealPath = Deno.realPathSync(`h/${packagePath}`);
if (!hRealPath.includes("_virtual_store")) {
  throw new Error(
    `expected peer hardlist entry not to disable global store: ${hRealPath}`,
  );
}

console.log("peer ignored");

const fPatchRealPath = Deno.realPathSync(
  "f/node_modules/@denotest/gvs-patch-target",
);
const gPatchRealPath = Deno.realPathSync(
  "g/node_modules/@denotest/gvs-patch-target",
);

if (fPatchRealPath === gPatchRealPath) {
  throw new Error(
    `expected patch content change to create a distinct package folder:\n${fPatchRealPath}`,
  );
}

for (const path of [fPatchRealPath, gPatchRealPath]) {
  if (!path.includes("_virtual_store")) {
    throw new Error(`expected patched package to use global store: ${path}`);
  }
}

const fPatchMain = Deno.readTextFileSync(`${fPatchRealPath}/main.js`);
const gPatchMain = Deno.readTextFileSync(`${gPatchRealPath}/main.js`);

if (fPatchMain !== 'module.exports = "one";\n') {
  throw new Error(`unexpected first patch content: ${fPatchMain}`);
}
if (gPatchMain !== 'module.exports = "two";\n') {
  throw new Error(`unexpected second patch content: ${gPatchMain}`);
}

console.log("patch content keyed");
