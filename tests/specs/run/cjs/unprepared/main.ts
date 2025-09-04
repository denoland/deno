// non-analyzable
const moduleName = "./output.cjs";
function getModuleName() {
  return moduleName;
}

await import(getModuleName());
