import vm from "node:vm";

function dynamicImportModule(value) {
  const module = new vm.SyntheticModule(["value"], () => {
    module.setExport("value", value);
  });
  return module;
}

function callback(label) {
  return (specifier, referrer, attributes) => {
    console.log(
      `${label} callback`,
      specifier,
      referrer?.constructor?.name,
      attributes.type,
    );
    return Promise.resolve(
      dynamicImportModule(`${label}:${specifier}:${attributes.type}`),
    );
  };
}

// new vm.Script(source, { importModuleDynamically })
{
  const script = new vm.Script(
    "globalThis.__script = import('vm:script', { with: { type: 'json' } }).then((m) => m.value)",
    { importModuleDynamically: callback("script") },
  );
  script.runInThisContext();
  console.log("Script", await globalThis.__script);
}

// vm.runInThisContext(source, { importModuleDynamically })
{
  vm.runInThisContext(
    "globalThis.__ritc = import('vm:ritc').then((m) => m.value)",
    { importModuleDynamically: callback("ritc") },
  );
  console.log("runInThisContext", await globalThis.__ritc);
}

// vm.runInContext(source, ctx, { importModuleDynamically })
{
  const context = vm.createContext({ Promise });
  vm.runInContext(
    "globalThis.value = import('vm:context').then((m) => m.value)",
    context,
    { importModuleDynamically: callback("context") },
  );
  console.log("runInContext", await context.value);
}

// vm.runInNewContext(source, sandbox, { importModuleDynamically })
{
  const sandbox = { Promise };
  vm.runInNewContext(
    "globalThis.value = import('vm:new-context').then((m) => m.value)",
    sandbox,
    { importModuleDynamically: callback("new-context") },
  );
  console.log("runInNewContext", await sandbox.value);
}

// vm.compileFunction(code, params, { importModuleDynamically })
{
  let referrer;
  const fn = vm.compileFunction("return import('vm:function');", [], {
    importModuleDynamically(specifier, ref, attributes) {
      referrer = ref;
      return callback("function")(specifier, ref, attributes);
    },
  });
  console.log("compileFunction", (await fn()).value, referrer === fn);
}

// vm.createContext(sandbox, { importModuleDynamically })
{
  const context = vm.createContext({ Promise }, {
    importModuleDynamically: callback("context-default"),
  });
  vm.runInContext(
    "globalThis.value = import('vm:context-default').then((m) => m.value)",
    context,
  );
  console.log("createContext", await context.value);
}

// new vm.SourceTextModule(source, { importModuleDynamically })
{
  let referrer;
  const module = new vm.SourceTextModule(
    "globalThis.__stm = import('vm:stm').then((m) => m.value)",
    {
      importModuleDynamically(specifier, ref, attributes) {
        referrer = ref;
        return callback("stm")(specifier, ref, attributes);
      },
    },
  );
  await module.link(() => {
    throw new Error("unexpected static import");
  });
  await module.evaluate();
  console.log("SourceTextModule", await globalThis.__stm, referrer === module);
}
