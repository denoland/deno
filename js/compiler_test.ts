// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import { test, assert, assertEqual } from "./test_util.ts";
import * as deno from "deno";
import * as ts from "typescript";

// We use a silly amount of `any` in these tests...
// tslint:disable:no-any

const { DenoCompiler, jsonAmdTemplate } = (deno as any)._compiler;

interface ModuleInfo {
  moduleName: string | undefined;
  filename: string | undefined;
  mediaType: MediaType | undefined;
  sourceCode: string | undefined;
  outputCode: string | undefined;
  sourceMap: string | undefined;
}

const compilerInstance = DenoCompiler.instance();

// References to original items we are going to mock
const originals = {
  _globalEval: (compilerInstance as any)._globalEval,
  _log: (compilerInstance as any)._log,
  _os: (compilerInstance as any)._os,
  _ts: (compilerInstance as any)._ts,
  _service: (compilerInstance as any)._service,
  _window: (compilerInstance as any)._window
};

enum MediaType {
  JavaScript = 0,
  TypeScript = 1,
  Json = 2,
  Unknown = 3
}

function mockModuleInfo(
  moduleName: string | undefined,
  filename: string | undefined,
  mediaType: MediaType | undefined,
  sourceCode: string | undefined,
  outputCode: string | undefined,
  sourceMap: string | undefined
): ModuleInfo {
  return {
    moduleName,
    filename,
    mediaType,
    sourceCode,
    outputCode,
    sourceMap
  };
}

// Some fixtures we will us in testing
const fooBarTsSource = `import * as deno from "deno";
console.log(deno);
export const foo = "bar";
`;

const fooBazTsSource = `import { foo } from "./bar.ts";
console.log(foo);
`;

const modASource = `import { B } from "./modB.ts";

export class A {
  b = new B();
};
`;

const modAModuleInfo = mockModuleInfo(
  "modA",
  "/root/project/modA.ts",
  MediaType.TypeScript,
  modASource,
  undefined,
  undefined
);

const modBSource = `import { A } from "./modA.ts";

export class B {
  a = new A();
};
`;

const modBModuleInfo = mockModuleInfo(
  "modB",
  "/root/project/modB.ts",
  MediaType.TypeScript,
  modBSource,
  undefined,
  undefined
);

// tslint:disable:max-line-length
const fooBarTsOutput = `define(["require", "exports", "deno"], function (require, exports, deno) {
    "use strict";
    Object.defineProperty(exports, "__esModule", { value: true });
    console.log(deno);
    exports.foo = "bar";
});
//# sourceMappingURL=bar.js.map
//# sourceURL=/root/project/foo/bar.ts`;

const fooBarTsSourcemap = `{"version":3,"file":"bar.js","sourceRoot":"","sources":["file:///root/project/foo/bar.ts"],"names":[],"mappings":";;;IACA,OAAO,CAAC,GAAG,CAAC,IAAI,CAAC,CAAC;IACL,QAAA,GAAG,GAAG,KAAK,CAAC"}`;

const fooBazTsOutput = `define(["require", "exports", "./bar.ts"], function (require, exports, bar_ts_1) {
  "use strict";
  Object.defineProperty(exports, "__esModule", { value: true });
  console.log(bar_ts_1.foo);
});
//# sourceMappingURL=baz.js.map
//# sourceURL=/root/project/foo/baz.ts`;

// This is not a valid map, just mock data
const fooBazTsSourcemap = `{"version":3,"file":"baz.js","sourceRoot":"","sources":["file:///root/project/foo/baz.ts"],"names":[],"mappings":""}`;

const loadConfigSource = `import * as config from "./config.json";
console.log(config.foo.baz);
`;
const configJsonSource = `{"foo":{"bar": true,"baz": ["qat", 1]}}`;
// tslint:enable:max-line-length

const moduleMap: {
  [containFile: string]: { [moduleSpecifier: string]: ModuleInfo };
} = {
  "/root/project": {
    "foo/bar.ts": mockModuleInfo(
      "/root/project/foo/bar.ts",
      "/root/project/foo/bar.ts",
      MediaType.TypeScript,
      fooBarTsSource,
      null,
      null
    ),
    "foo/baz.ts": mockModuleInfo(
      "/root/project/foo/baz.ts",
      "/root/project/foo/baz.ts",
      MediaType.TypeScript,
      fooBazTsSource,
      fooBazTsOutput,
      fooBazTsSourcemap
    ),
    "modA.ts": modAModuleInfo,
    "some.txt": mockModuleInfo(
      "/root/project/some.txt",
      "/root/project/some.text",
      MediaType.Unknown,
      "console.log();",
      null,
      null
    ),
    "loadConfig.ts": mockModuleInfo(
      "/root/project/loadConfig.ts",
      "/root/project/loadConfig.ts",
      MediaType.TypeScript,
      loadConfigSource,
      null,
      null
    )
  },
  "/root/project/foo/baz.ts": {
    "./bar.ts": mockModuleInfo(
      "/root/project/foo/bar.ts",
      "/root/project/foo/bar.ts",
      MediaType.TypeScript,
      fooBarTsSource,
      fooBarTsOutput,
      fooBarTsSourcemap
    )
  },
  "/root/project/modA.ts": {
    "./modB.ts": modBModuleInfo
  },
  "/root/project/modB.ts": {
    "./modA.ts": modAModuleInfo
  },
  "/root/project/loadConfig.ts": {
    "./config.json": mockModuleInfo(
      "/root/project/config.json",
      "/root/project/config.json",
      MediaType.Json,
      configJsonSource,
      null,
      null
    )
  },
  "/moduleKinds": {
    "foo.ts": mockModuleInfo(
      "foo",
      "/moduleKinds/foo.ts",
      MediaType.TypeScript,
      "console.log('foo');",
      undefined,
      undefined
    ),
    "foo.d.ts": mockModuleInfo(
      "foo",
      "/moduleKinds/foo.d.ts",
      MediaType.TypeScript,
      "console.log('foo');",
      undefined,
      undefined
    ),
    "foo.js": mockModuleInfo(
      "foo",
      "/moduleKinds/foo.js",
      MediaType.JavaScript,
      "console.log('foo');",
      undefined,
      undefined
    ),
    "foo.json": mockModuleInfo(
      "foo",
      "/moduleKinds/foo.json",
      MediaType.Json,
      "console.log('foo');",
      undefined,
      undefined
    ),
    "foo.txt": mockModuleInfo(
      "foo",
      "/moduleKinds/foo.txt",
      MediaType.JavaScript,
      "console.log('foo');",
      undefined,
      undefined
    )
  }
};

const moduleCache: {
  [fileName: string]: ModuleInfo;
} = {
  "/root/project/modA.ts": modAModuleInfo,
  "/root/project/modB.ts": modBModuleInfo
};

const emittedFiles = {
  "/root/project/foo/qat.ts": "console.log('foo');"
};

let globalEvalStack: string[] = [];
let getEmitOutputStack: string[] = [];
let logStack: any[][] = [];
let codeCacheStack: Array<{
  fileName: string;
  sourceCode: string;
  outputCode: string;
  sourceMap: string;
}> = [];
let codeFetchStack: Array<{
  moduleSpecifier: string;
  containingFile: string;
}> = [];

let mockDepsStack: string[][] = [];
let mockFactoryStack: any[] = [];

function globalEvalMock(x: string): void {
  globalEvalStack.push(x);
  if (windowMock.define && mockDepsStack.length && mockFactoryStack.length) {
    windowMock.define(mockDepsStack.pop(), mockFactoryStack.pop());
  }
}
function logMock(...args: any[]): void {
  logStack.push(args);
}
const osMock = {
  codeCache(
    fileName: string,
    sourceCode: string,
    outputCode: string,
    sourceMap: string
  ): void {
    codeCacheStack.push({ fileName, sourceCode, outputCode, sourceMap });
    if (fileName in moduleCache) {
      moduleCache[fileName].sourceCode = sourceCode;
      moduleCache[fileName].outputCode = outputCode;
      moduleCache[fileName].sourceMap = sourceMap;
    } else {
      moduleCache[fileName] = mockModuleInfo(
        fileName,
        fileName,
        MediaType.TypeScript,
        sourceCode,
        outputCode,
        sourceMap
      );
    }
  },
  codeFetch(moduleSpecifier: string, containingFile: string): ModuleInfo {
    codeFetchStack.push({ moduleSpecifier, containingFile });
    if (containingFile in moduleMap) {
      if (moduleSpecifier in moduleMap[containingFile]) {
        return moduleMap[containingFile][moduleSpecifier];
      }
    }
    return mockModuleInfo(null, null, null, null, null, null);
  },
  exit(code: number): never {
    throw new Error(`Unexpected call to os.exit(${code})`);
  }
};
const tsMock = {
  createLanguageService(host: ts.LanguageServiceHost): ts.LanguageService {
    return {} as ts.LanguageService;
  },
  formatDiagnosticsWithColorAndContext(
    diagnostics: ReadonlyArray<ts.Diagnostic>,
    _host: ts.FormatDiagnosticsHost
  ): string {
    return JSON.stringify(diagnostics.map(({ messageText }) => messageText));
  }
};

const getEmitOutputPassThrough = true;

const serviceMock = {
  getCompilerOptionsDiagnostics(): ts.Diagnostic[] {
    return originals._service.getCompilerOptionsDiagnostics.call(
      originals._service
    );
  },
  getEmitOutput(fileName: string): ts.EmitOutput {
    getEmitOutputStack.push(fileName);
    if (getEmitOutputPassThrough) {
      return originals._service.getEmitOutput.call(
        originals._service,
        fileName
      );
    }
    if (fileName in emittedFiles) {
      return {
        outputFiles: [{ text: emittedFiles[fileName] }] as any,
        emitSkipped: false
      };
    }
    return { outputFiles: [], emitSkipped: false };
  },
  getSemanticDiagnostics(fileName: string): ts.Diagnostic[] {
    return originals._service.getSemanticDiagnostics.call(
      originals._service,
      fileName
    );
  },
  getSyntacticDiagnostics(fileName: string): ts.Diagnostic[] {
    return originals._service.getSyntacticDiagnostics.call(
      originals._service,
      fileName
    );
  }
};
const windowMock: { define?: any } = {};
const mocks = {
  _globalEval: globalEvalMock,
  _log: logMock,
  _os: osMock,
  _ts: tsMock,
  _service: serviceMock,
  _window: windowMock
};

/**
 * Setup the mocks for a test
 */
function setup() {
  // monkey patch mocks on instance
  Object.assign(compilerInstance, mocks);
}

/**
 * Teardown the mocks for a test
 */
function teardown() {
  // reset compiler internal state
  (compilerInstance as any)._moduleMetaDataMap.clear();
  (compilerInstance as any)._fileNamesMap.clear();

  // reset mock states
  codeFetchStack = [];
  codeCacheStack = [];
  logStack = [];
  getEmitOutputStack = [];
  globalEvalStack = [];

  assertEqual(mockDepsStack.length, 0);
  assertEqual(mockFactoryStack.length, 0);
  mockDepsStack = [];
  mockFactoryStack = [];

  // restore original properties and methods
  Object.assign(compilerInstance, originals);
}

test(function testJsonAmdTemplate() {
  let deps: string[];
  let factory: Function;
  function define(d: string[], f: Function) {
    deps = d;
    factory = f;
  }

  const code = jsonAmdTemplate(`{ "hello": "world", "foo": "bar" }`);
  const result = eval(code);
  assert(result == null);
  assertEqual(deps && deps.length, 0);
  assert(factory != null);
  const factoryResult = factory();
  assertEqual(factoryResult, { hello: "world", foo: "bar" });
});

test(function compilerInstance() {
  assert(DenoCompiler != null);
  assert(DenoCompiler.instance() != null);
});

// Testing the internal APIs

test(function compilerRun() {
  // equal to `deno foo/bar.ts`
  setup();
  let factoryRun = false;
  mockDepsStack.push(["require", "exports", "deno"]);
  mockFactoryStack.push((_require, _exports, _deno) => {
    factoryRun = true;
    assertEqual(typeof _require, "function");
    assertEqual(typeof _exports, "object");
    assert(_deno === deno);
    _exports.foo = "bar";
  });
  const moduleMetaData = compilerInstance.run("foo/bar.ts", "/root/project");
  assert(factoryRun);
  assert(moduleMetaData.hasRun);
  assertEqual(moduleMetaData.sourceCode, fooBarTsSource);
  assertEqual(moduleMetaData.outputCode, fooBarTsOutput);
  assertEqual(moduleMetaData.sourceMap, fooBarTsSourcemap);
  assertEqual(moduleMetaData.exports, { foo: "bar" });

  assertEqual(
    codeFetchStack.length,
    1,
    "Module should have only been fetched once."
  );
  assertEqual(
    codeCacheStack.length,
    1,
    "Compiled code should have only been cached once."
  );
  const [codeCacheCall] = codeCacheStack;
  assertEqual(codeCacheCall.fileName, "/root/project/foo/bar.ts");
  assertEqual(codeCacheCall.sourceCode, fooBarTsSource);
  assertEqual(codeCacheCall.outputCode, fooBarTsOutput);
  assertEqual(codeCacheCall.sourceMap, fooBarTsSourcemap);
  teardown();
});

test(function compilerRunMultiModule() {
  // equal to `deno foo/baz.ts`
  setup();
  const factoryStack: string[] = [];
  const bazDeps = ["require", "exports", "./bar.ts"];
  const bazFactory = (_require, _exports, _bar) => {
    factoryStack.push("baz");
    assertEqual(_bar.foo, "bar");
  };
  const barDeps = ["require", "exports", "deno"];
  const barFactory = (_require, _exports, _deno) => {
    factoryStack.push("bar");
    _exports.foo = "bar";
  };
  mockDepsStack.push(barDeps);
  mockFactoryStack.push(barFactory);
  mockDepsStack.push(bazDeps);
  mockFactoryStack.push(bazFactory);
  compilerInstance.run("foo/baz.ts", "/root/project");
  assertEqual(factoryStack, ["bar", "baz"]);

  assertEqual(
    codeFetchStack.length,
    2,
    "Modules should have only been fetched once."
  );
  assertEqual(codeCacheStack.length, 0, "No code should have been cached.");
  teardown();
});

test(function compilerRunCircularDependency() {
  setup();
  const factoryStack: string[] = [];
  const modADeps = ["require", "exports", "./modB.ts"];
  const modAFactory = (_require, _exports, _modB) => {
    assertEqual(_modB.foo, "bar");
    factoryStack.push("modA");
    _exports.bar = "baz";
    _modB.assertModA();
  };
  const modBDeps = ["require", "exports", "./modA.ts"];
  const modBFactory = (_require, _exports, _modA) => {
    assertEqual(_modA, {});
    factoryStack.push("modB");
    _exports.foo = "bar";
    _exports.assertModA = () => {
      assertEqual(_modA, {
        bar: "baz"
      });
    };
  };
  mockDepsStack.push(modBDeps);
  mockFactoryStack.push(modBFactory);
  mockDepsStack.push(modADeps);
  mockFactoryStack.push(modAFactory);
  compilerInstance.run("modA.ts", "/root/project");
  assertEqual(factoryStack, ["modB", "modA"]);
  teardown();
});

test(function compilerLoadJsonModule() {
  setup();
  const factoryStack: string[] = [];
  const configJsonDeps: string[] = [];
  const configJsonFactory = () => {
    factoryStack.push("configJson");
    return JSON.parse(configJsonSource);
  };
  const loadConfigDeps = ["require", "exports", "./config.json"];
  const loadConfigFactory = (_require, _exports, _config) => {
    factoryStack.push("loadConfig");
    assertEqual(_config, JSON.parse(configJsonSource));
  };

  mockDepsStack.push(configJsonDeps);
  mockFactoryStack.push(configJsonFactory);
  mockDepsStack.push(loadConfigDeps);
  mockFactoryStack.push(loadConfigFactory);
  compilerInstance.run("loadConfig.ts", "/root/project");
  assertEqual(factoryStack, ["configJson", "loadConfig"]);
  teardown();
});

test(function compilerResolveModule() {
  setup();
  const moduleMetaData = compilerInstance.resolveModule(
    "foo/baz.ts",
    "/root/project"
  );
  assertEqual(moduleMetaData.sourceCode, fooBazTsSource);
  assertEqual(moduleMetaData.outputCode, fooBazTsOutput);
  assertEqual(moduleMetaData.sourceMap, fooBazTsSourcemap);
  assert(!moduleMetaData.hasRun);
  assert(!moduleMetaData.deps);
  assertEqual(moduleMetaData.exports, {});
  assertEqual(moduleMetaData.scriptVersion, "1");

  assertEqual(codeFetchStack.length, 1, "Only initial module is resolved.");
  teardown();
});

test(function compilerResolveModuleUnknownMediaType() {
  setup();
  let didThrow = false;
  try {
    compilerInstance.resolveModule("some.txt", "/root/project");
  } catch (e) {
    assert(e instanceof Error);
    assertEqual(
      e.message,
      `Unknown media type for: "some.txt" from "/root/project".`
    );
    didThrow = true;
  }
  assert(didThrow);
  teardown();
});

test(function compilerGetModuleDependencies() {
  setup();
  const bazDeps = ["require", "exports", "./bar.ts"];
  const bazFactory = () => {
    throw new Error("Unexpected factory call");
  };
  const barDeps = ["require", "exports", "deno"];
  const barFactory = () => {
    throw new Error("Unexpected factory call");
  };
  mockDepsStack.push(barDeps);
  mockFactoryStack.push(barFactory);
  mockDepsStack.push(bazDeps);
  mockFactoryStack.push(bazFactory);
  const deps = compilerInstance.getModuleDependencies(
    "foo/baz.ts",
    "/root/project"
  );
  assertEqual(deps, ["/root/project/foo/bar.ts", "/root/project/foo/baz.ts"]);
  teardown();
});

// TypeScript LanguageServiceHost APIs

test(function compilerGetCompilationSettings() {
  const expectedKeys = [
    "allowJs",
    "checkJs",
    "module",
    "outDir",
    "resolveJsonModule",
    "sourceMap",
    "stripComments",
    "target"
  ];
  const result = compilerInstance.getCompilationSettings();
  for (const key of expectedKeys) {
    assert(key in result, `Expected "${key}" in compiler options.`);
  }
  assertEqual(Object.keys(result).length, expectedKeys.length);
});

test(function compilerGetNewLine() {
  const result = compilerInstance.getNewLine();
  assertEqual(result, "\n", "Expected newline value of '\\n'.");
});

test(function compilerGetScriptFileNames() {
  setup();
  compilerInstance.run("foo/bar.ts", "/root/project");
  const result = compilerInstance.getScriptFileNames();
  assertEqual(result.length, 1, "Expected only a single filename.");
  assertEqual(result[0], "/root/project/foo/bar.ts");
  teardown();
});

test(function compilerRecompileFlag() {
  setup();
  compilerInstance.run("foo/bar.ts", "/root/project");
  assertEqual(
    getEmitOutputStack.length,
    1,
    "Expected only a single emitted file."
  );
  // running compiler against same file should use cached code
  compilerInstance.run("foo/bar.ts", "/root/project");
  assertEqual(
    getEmitOutputStack.length,
    1,
    "Expected only a single emitted file."
  );
  compilerInstance.recompile = true;
  compilerInstance.run("foo/bar.ts", "/root/project");
  assertEqual(getEmitOutputStack.length, 2, "Expected two emitted file.");
  assert(
    getEmitOutputStack[0] === getEmitOutputStack[1],
    "Expected same file to be emitted twice."
  );
  teardown();
});

test(function compilerGetScriptKind() {
  setup();
  compilerInstance.resolveModule("foo.ts", "/moduleKinds");
  compilerInstance.resolveModule("foo.d.ts", "/moduleKinds");
  compilerInstance.resolveModule("foo.js", "/moduleKinds");
  compilerInstance.resolveModule("foo.json", "/moduleKinds");
  compilerInstance.resolveModule("foo.txt", "/moduleKinds");
  assertEqual(
    compilerInstance.getScriptKind("/moduleKinds/foo.ts"),
    ts.ScriptKind.TS
  );
  assertEqual(
    compilerInstance.getScriptKind("/moduleKinds/foo.d.ts"),
    ts.ScriptKind.TS
  );
  assertEqual(
    compilerInstance.getScriptKind("/moduleKinds/foo.js"),
    ts.ScriptKind.JS
  );
  assertEqual(
    compilerInstance.getScriptKind("/moduleKinds/foo.json"),
    ts.ScriptKind.JSON
  );
  assertEqual(
    compilerInstance.getScriptKind("/moduleKinds/foo.txt"),
    ts.ScriptKind.JS
  );
  teardown();
});

test(function compilerGetScriptVersion() {
  setup();
  const moduleMetaData = compilerInstance.resolveModule(
    "foo/bar.ts",
    "/root/project"
  );
  compilerInstance.compile(moduleMetaData);
  assertEqual(
    compilerInstance.getScriptVersion(moduleMetaData.fileName),
    "1",
    "Expected known module to have script version of 1"
  );
  teardown();
});

test(function compilerGetScriptVersionUnknown() {
  assertEqual(
    compilerInstance.getScriptVersion("/root/project/unknown_module.ts"),
    "",
    "Expected unknown module to have an empty script version"
  );
});

test(function compilerGetScriptSnapshot() {
  setup();
  const moduleMetaData = compilerInstance.resolveModule(
    "foo/bar.ts",
    "/root/project"
  );
  const result = compilerInstance.getScriptSnapshot(moduleMetaData.fileName);
  assert(result != null, "Expected snapshot to be defined.");
  assertEqual(result.getLength(), fooBarTsSource.length);
  assertEqual(
    result.getText(0, 6),
    "import",
    "Expected .getText() to equal 'import'"
  );
  assertEqual(result.getChangeRange(result), undefined);
  // This is and optional part of the `IScriptSnapshot` API which we don't
  // define, os checking for the lack of this property.
  assert(!("dispose" in result));

  assert(
    result === moduleMetaData,
    "result should strictly equal moduleMetaData"
  );
  teardown();
});

test(function compilerGetCurrentDirectory() {
  assertEqual(compilerInstance.getCurrentDirectory(), "");
});

test(function compilerGetDefaultLibFileName() {
  setup();
  assertEqual(
    compilerInstance.getDefaultLibFileName(),
    "$asset$/lib.deno_runtime.d.ts"
  );
  teardown();
});

test(function compilerUseCaseSensitiveFileNames() {
  assertEqual(compilerInstance.useCaseSensitiveFileNames(), true);
});

test(function compilerReadFile() {
  let doesThrow = false;
  try {
    compilerInstance.readFile("foobar.ts");
  } catch (e) {
    doesThrow = true;
    assert(e.message.includes("Not implemented") === true);
  }
  assert(doesThrow);
});

test(function compilerFileExists() {
  setup();
  const moduleMetaData = compilerInstance.resolveModule(
    "foo/bar.ts",
    "/root/project"
  );
  assert(compilerInstance.fileExists(moduleMetaData.fileName));
  assert(compilerInstance.fileExists("$asset$/lib.deno_runtime.d.ts"));
  assertEqual(
    compilerInstance.fileExists("/root/project/unknown-module.ts"),
    false
  );
  teardown();
});

test(function compilerResolveModuleNames() {
  setup();
  const results = compilerInstance.resolveModuleNames(
    ["foo/bar.ts", "foo/baz.ts", "deno"],
    "/root/project"
  );
  assertEqual(results.length, 3);
  const fixtures: Array<[string, boolean]> = [
    ["/root/project/foo/bar.ts", false],
    ["/root/project/foo/baz.ts", false],
    ["$asset$/lib.deno_runtime.d.ts", true]
  ];
  for (let i = 0; i < results.length; i++) {
    const result = results[i];
    const [resolvedFileName, isExternalLibraryImport] = fixtures[i];
    assertEqual(result.resolvedFileName, resolvedFileName);
    assertEqual(result.isExternalLibraryImport, isExternalLibraryImport);
  }
  teardown();
});
