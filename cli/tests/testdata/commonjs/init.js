const DENO_NODE_COMPAT_URL = Deno.env.get("DENO_NODE_COMPAT_URL");
const moduleAllUrl = `${DENO_NODE_COMPAT_URL}node/module_all.ts`;
const processUrl = `${DENO_NODE_COMPAT_URL}node/process.ts`;
// remove file:// prefix
const moduleName = import.meta.resolve(Deno.args[0]).slice(6);
console.log(moduleName);

const [moduleAll, processModule] = await Promise.all([
  import(moduleAllUrl),
  import(processUrl),
]);
Deno[Deno.internal].require.initializeCommonJs(
  moduleAll.default,
  processModule.default,
);
Deno[Deno.internal].require.Module._load(moduleName, null, true);
