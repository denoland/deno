// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import { test, assert, assertEqual } from "./test_util.ts";
import * as compiler from "compiler";
import * as ts from "typescript";

// We use a silly amount of `any` in these tests...
// tslint:disable:no-any

const { DenoCompiler } = compiler;

// Enums like this don't exist at runtime, so local copy
enum ScriptKind {
  JS = 1,
  TS = 3,
  JSON = 6
}

interface ModuleInfo {
  moduleName: string | null;
  filename: string | null;
  sourceCode: string | null;
  outputCode: string | null;
}

const compilerInstance = DenoCompiler.instance();

// References to orignal items we are going to mock
const originals = {
  _globalEval: (compilerInstance as any)._globalEval,
  _log: (compilerInstance as any)._log,
  _os: (compilerInstance as any)._os,
  _ts: (compilerInstance as any)._ts,
  _service: (compilerInstance as any)._service,
  _window: (compilerInstance as any)._window
};

function mockModuleInfo(
  moduleName: string | null,
  filename: string | null,
  sourceCode: string | null,
  outputCode: string | null
): ModuleInfo {
  return {
    moduleName,
    filename,
    sourceCode,
    outputCode
  };
}

// Some fixtures we will us in testing
const fooBarTsSource = `import * as compiler from "compiler";
console.log(compiler);
export const foo = "bar";
`;

const fooBazTsSource = `import { foo } from "./bar.ts";
console.log(foo);
`;

// TODO(#23) Remove source map strings from fooBarTsOutput.
// tslint:disable:max-line-length
const fooBarTsOutput = `define(["require", "exports", "compiler"], function (require, exports, compiler) {
    "use strict";
    Object.defineProperty(exports, "__esModule", { value: true });
    console.log(compiler);
    exports.foo = "bar";
});
//# sourceMappingURL=data:application/json;base64,eyJ2ZXJzaW9uIjozLCJmaWxlIjoiYmFyLmpzIiwic291cmNlUm9vdCI6IiIsInNvdXJjZXMiOlsiZmlsZTovLy9yb290L3Byb2plY3QvZm9vL2Jhci50cyJdLCJuYW1lcyI6W10sIm1hcHBpbmdzIjoiOzs7SUFDQSxPQUFPLENBQUMsR0FBRyxDQUFDLFFBQVEsQ0FBQyxDQUFDO0lBQ1QsUUFBQSxHQUFHLEdBQUcsS0FBSyxDQUFDIiwic291cmNlc0NvbnRlbnQiOlsiaW1wb3J0ICogYXMgY29tcGlsZXIgZnJvbSBcImNvbXBpbGVyXCI7XG5jb25zb2xlLmxvZyhjb21waWxlcik7XG5leHBvcnQgY29uc3QgZm9vID0gXCJiYXJcIjtcbiJdfQ==
//# sourceURL=/root/project/foo/bar.ts`;

// TODO(#23) Remove source map strings from fooBazTsOutput.
const fooBazTsOutput = `define(["require", "exports", "./bar.ts"], function (require, exports, bar_ts_1) {
  "use strict";
  Object.defineProperty(exports, "__esModule", { value: true });
  console.log(bar_ts_1.foo);
});
//# sourceMappingURL=data:application/json;base64,eyJ2ZXJzaW9uIjozLCJmaWxlIjoiYmF6LmpzIiwic291cmNlUm9vdCI6IiIsInNvdXJjZXMiOlsiZmlsZTovLy9yb290L3Byb2plY3QvZm9vL2Jhei50cyJdLCJuYW1lcyI6W10sIm1hcHBpbmdzIjoiOzs7SUFDQSxPQUFPLENBQUMsR0FBRyxDQUFDLFlBQUcsQ0FBQyxDQUFDIiwic291cmNlc0NvbnRlbnQiOlsiaW1wb3J0IHsgZm9vIH0gZnJvbSBcIi4vYmFyLnRzXCI7XG5jb25zb2xlLmxvZyhmb28pO1xuIl19
//# sourceURL=/root/project/foo/baz.ts`;
// tslint:enable:max-line-length

const moduleMap: {
  [containFile: string]: { [moduleSpecifier: string]: ModuleInfo };
} = {
  "/root/project": {
    "foo/bar.ts": mockModuleInfo(
      "foo/bar",
      "/root/project/foo/bar.ts",
      fooBarTsSource,
      null
    ),
    "foo/baz.ts": mockModuleInfo(
      "foo/baz",
      "/root/project/foo/baz.ts",
      fooBazTsSource,
      fooBazTsOutput
    )
  },
  "/root/project/foo/baz.ts": {
    "./bar.ts": mockModuleInfo(
      "foo/bar",
      "/root/project/foo/bar.ts",
      fooBarTsSource,
      fooBarTsOutput
    )
  }
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
}> = [];
let codeFetchStack: Array<{
  moduleSpecifier: string;
  containingFile: string;
}> = [];

let mockDepsStack: string[][] = [];
let mockFactoryStack: compiler.AmdFactory[] = [];

function globalEvalMock(x: string): void {
  globalEvalStack.push(x);
  if (windowMock.define && mockDepsStack.length && mockFactoryStack.length) {
    windowMock.define(mockDepsStack.pop(), mockFactoryStack.pop());
  }
}
function logMock(...args: any[]): void {
  logStack.push(args);
}
const osMock: compiler.Os = {
  codeCache(fileName: string, sourceCode: string, outputCode: string): void {
    codeCacheStack.push({ fileName, sourceCode, outputCode });
  },
  codeFetch(moduleSpecifier: string, containingFile: string): ModuleInfo {
    codeFetchStack.push({ moduleSpecifier, containingFile });
    if (containingFile in moduleMap) {
      if (moduleSpecifier in moduleMap[containingFile]) {
        return moduleMap[containingFile][moduleSpecifier];
      }
    }
    return mockModuleInfo(null, null, null, null);
  },
  exit(code: number): never {
    throw new Error(`os.exit(${code})`);
  }
};
const tsMock: compiler.Ts = {
  createLanguageService(host: ts.LanguageServiceHost): ts.LanguageService {
    return {} as ts.LanguageService;
  },
  formatDiagnosticsWithColorAndContext(
    diagnostics: ReadonlyArray<ts.Diagnostic>,
    host: ts.FormatDiagnosticsHost
  ): string {
    return "";
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
const windowMock: { define?: compiler.AmdDefine } = {};
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

test(function compilerInstance() {
  assert(DenoCompiler != null);
  assert(DenoCompiler.instance() != null);
});

// Testing the internal APIs

test(function compilerRun() {
  // equal to `deno foo/bar.ts`
  setup();
  let factoryRun = false;
  mockDepsStack.push(["require", "exports", "compiler"]);
  mockFactoryStack.push((_require, _exports, _compiler) => {
    factoryRun = true;
    assertEqual(typeof _require, "function");
    assertEqual(typeof _exports, "object");
    assert(_compiler === compiler);
    _exports.foo = "bar";
  });
  const moduleMetaData = compilerInstance.run("foo/bar.ts", "/root/project");
  assert(factoryRun);
  assert(moduleMetaData.hasRun);
  assertEqual(moduleMetaData.sourceCode, fooBarTsSource);
  assertEqual(moduleMetaData.outputCode, fooBarTsOutput);
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
  const barDeps = ["require", "exports", "compiler"];
  const barFactory = (_require, _exports, _compiler) => {
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

test(function compilerResolveModule() {
  setup();
  const moduleMetaData = compilerInstance.resolveModule(
    "foo/baz.ts",
    "/root/project"
  );
  assertEqual(moduleMetaData.sourceCode, fooBazTsSource);
  assertEqual(moduleMetaData.outputCode, fooBazTsOutput);
  assert(!moduleMetaData.hasRun);
  assert(!moduleMetaData.deps);
  assertEqual(moduleMetaData.exports, {});
  assertEqual(moduleMetaData.scriptVersion, "1");

  assertEqual(codeFetchStack.length, 1, "Only initial module is resolved.");
  teardown();
});

test(function compilerGetModuleDependencies() {
  setup();
  const bazDeps = ["require", "exports", "./bar.ts"];
  const bazFactory = () => {
    throw new Error("Unexpected factory call");
  };
  const barDeps = ["require", "exports", "compiler"];
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
  const result = compilerInstance.getCompilationSettings();
  for (const key of [
    "allowJs",
    "module",
    "outDir",
    "inlineSourceMap",
    "inlineSources",
    "stripComments",
    "target"
  ]) {
    assert(key in result, `Expected "${key}" in compiler options.`);
  }
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

test(function compilerGetScriptKind() {
  assertEqual(compilerInstance.getScriptKind("foo.ts"), ScriptKind.TS);
  assertEqual(compilerInstance.getScriptKind("foo.d.ts"), ScriptKind.TS);
  assertEqual(compilerInstance.getScriptKind("foo.js"), ScriptKind.JS);
  assertEqual(compilerInstance.getScriptKind("foo.json"), ScriptKind.JSON);
  assertEqual(compilerInstance.getScriptKind("foo.txt"), ScriptKind.JS);
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
    "$asset$/lib.globals.d.ts"
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
  assert(compilerInstance.fileExists("$asset$/globals.d.ts"));
  assertEqual(
    compilerInstance.fileExists("/root/project/unknown-module.ts"),
    false
  );
  teardown();
});

test(function compilerResolveModuleNames() {
  setup();
  const results = compilerInstance.resolveModuleNames(
    ["foo/bar.ts", "foo/baz.ts", "$asset$/lib.globals.d.ts", "deno"],
    "/root/project"
  );
  assertEqual(results.length, 4);
  const fixtures: Array<[string, boolean]> = [
    ["/root/project/foo/bar.ts", false],
    ["/root/project/foo/baz.ts", false],
    ["$asset$/lib.globals.d.ts", true],
    ["$asset$/globals.d.ts", true]
  ];
  for (let i = 0; i < results.length; i++) {
    const result = results[i];
    const [resolvedFileName, isExternalLibraryImport] = fixtures[i];
    assertEqual(result.resolvedFileName, resolvedFileName);
    assertEqual(result.isExternalLibraryImport, isExternalLibraryImport);
  }
  teardown();
});
