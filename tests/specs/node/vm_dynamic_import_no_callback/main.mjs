// Reproduces denoland/deno#33385: dynamic `import()` inside `node:vm` code
// must reject with ERR_VM_DYNAMIC_IMPORT_CALLBACK_MISSING when the script
// was compiled without an `importModuleDynamically` callback.

import vm from "node:vm";

const EXPECTED_CODE = "ERR_VM_DYNAMIC_IMPORT_CALLBACK_MISSING";

const dynImportSrc =
  `(async () => { const $import = new Function('p', 'return import(p)'); ` +
  `try { await $import('node:fs'); globalThis.__result = 'unexpected success'; } ` +
  `catch (e) { globalThis.__result = { code: e.code, name: e.name, msg: e.message }; } ` +
  `})();`;

function check(label, result) {
  if (typeof result !== "object" || result === null) {
    console.log(label, "FAIL: not an object", result);
    return;
  }
  const ok = result.code === EXPECTED_CODE &&
    result.name === "TypeError" &&
    result.msg === "A dynamic import callback was not specified.";
  console.log(label, ok ? "ok" : "FAIL", JSON.stringify(result));
}

async function waitFor(getter) {
  for (let i = 0; i < 50; i++) {
    if (getter() !== undefined) return;
    await new Promise((r) => setTimeout(r, 10));
  }
}

// runInThisContext
delete globalThis.__result;
vm.runInThisContext(dynImportSrc, { filename: "rits.mjs" });
await waitFor(() => globalThis.__result);
check("runInThisContext", globalThis.__result);

// runInContext / createContext
delete globalThis.__result;
const ctx = vm.createContext({
  setTimeout,
  Function,
  Promise,
  get __result() {
    return globalThis.__result;
  },
  set __result(v) {
    globalThis.__result = v;
  },
});
vm.runInContext(dynImportSrc, ctx, { filename: "ric.mjs" });
await waitFor(() => globalThis.__result);
check("runInContext", globalThis.__result);

// runInNewContext
delete globalThis.__result;
const newCtx = {
  setTimeout,
  Function,
  Promise,
  get __result() {
    return globalThis.__result;
  },
  set __result(v) {
    globalThis.__result = v;
  },
};
vm.runInNewContext(dynImportSrc, newCtx, { filename: "rinc.mjs" });
await waitFor(() => globalThis.__result);
check("runInNewContext", globalThis.__result);

// new vm.Script(...).runInThisContext()
delete globalThis.__result;
const script = new vm.Script(dynImportSrc, { filename: "script.mjs" });
script.runInThisContext();
await waitFor(() => globalThis.__result);
check("Script.runInThisContext", globalThis.__result);

// vm.compileFunction
{
  const fn = vm.compileFunction("return import(p);", ["p"], {
    filename: "cf.mjs",
  });
  try {
    await fn("node:fs");
    console.log("compileFunction FAIL: unexpected success");
  } catch (e) {
    check("compileFunction", {
      code: e.code,
      name: e.name,
      msg: e.message,
    });
  }
}

// vm.SourceTextModule
{
  delete globalThis.__result;
  const m = new vm.SourceTextModule(dynImportSrc, { identifier: "stm" });
  await m.link(() => {
    throw new Error("no deps expected");
  });
  await m.evaluate();
  await waitFor(() => globalThis.__result);
  check("SourceTextModule", globalThis.__result);
}

// USE_MAIN_CONTEXT_DEFAULT_LOADER: dynamic import should *succeed*
{
  const src =
    `globalThis.__loader_ok = import('node:process').then((m) => typeof m.argv);`;
  const script = new vm.Script(src, {
    filename: "loader.mjs",
    importModuleDynamically: vm.constants.USE_MAIN_CONTEXT_DEFAULT_LOADER,
  });
  script.runInThisContext();
  const ty = await globalThis.__loader_ok;
  console.log(
    "USE_MAIN_CONTEXT_DEFAULT_LOADER",
    ty === "object" ? "ok" : `FAIL got ${ty}`,
  );
}
