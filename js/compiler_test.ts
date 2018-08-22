// Copyright 2018 the Deno authors. All rights reserved. MIT license.
import { test, assert, assertEqual } from "./testing/testing.ts";
import * as compiler from "compiler";
import * as ts from "typescript";

// We use a silly amount of `any` in these tests...
// tslint:disable:no-any

const { DenoCompiler, ModuleMetaData } = compiler;

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
      null
    ),
    "foo/qat.ts": mockModuleInfo(
      "foo/qat",
      "/root/project/foo/qat.ts",
      null,
      null
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

function reset() {
  codeFetchStack = [];
  codeCacheStack = [];
  logStack = [];
  getEmitOutputStack = [];
  globalEvalStack = [];
}

let mockDeps: string[] | undefined;
let mockFactory: compiler.AmdFactory;

function globalEvalMock(x: string): void {
  globalEvalStack.push(x);
  if (windowMock.define && mockDeps && mockFactory) {
    windowMock.define(mockDeps, mockFactory);
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

// Setup the mocks
test(function compilerTestsSetup() {
  assert("_globalEval" in compilerInstance);
  assert("_log" in compilerInstance);
  assert("_os" in compilerInstance);
  assert("_ts" in compilerInstance);
  assert("_service" in compilerInstance);
  assert("_window" in compilerInstance);
  Object.assign(compilerInstance, mocks);
});

test(function compilerInstance() {
  assert(DenoCompiler != null);
  assert(DenoCompiler.instance() != null);
});

// Testing the internal APIs

test(function compilerMakeDefine() {
  const moduleMetaData = new ModuleMetaData(
    "/root/project/foo/bar.ts",
    fooBarTsSource,
    fooBarTsOutput
  );
  const localDefine = compilerInstance.makeDefine(moduleMetaData);
  let factoryCalled = false;
  localDefine(
    ["require", "exports", "compiler"],
    (_require, _exports, _compiler): void => {
      factoryCalled = true;
      assertEqual(
        typeof _require,
        "function",
        "localRequire should be a function"
      );
      assert(_exports != null);
      assert(
        Object.keys(_exports).length === 0,
        "exports should have no properties"
      );
      assert(compiler === _compiler, "compiler should be passed to factory");
    }
  );
  assert(factoryCalled, "Factory expected to be called");
});

// TODO testMakeDefineExternalModule - testing that make define properly runs
// external modules, this is implicitly tested though in
// `compilerRunMultiModule`

test(function compilerRun() {
  // equal to `deno foo/bar.ts`
  reset();
  const result = compilerInstance.run("foo/bar.ts", "/root/project");
  assert(result instanceof ModuleMetaData);
  assertEqual(codeFetchStack.length, 1);
  assertEqual(codeCacheStack.length, 1);
  assertEqual(globalEvalStack.length, 1);

  const lastGlobalEval = globalEvalStack.pop();
  assertEqual(lastGlobalEval, fooBarTsOutput);
  const lastCodeFetch = codeFetchStack.pop();
  assertEqual(lastCodeFetch, {
    moduleSpecifier: "foo/bar.ts",
    containingFile: "/root/project"
  });
  const lastCodeCache = codeCacheStack.pop();
  assertEqual(lastCodeCache, {
    fileName: "/root/project/foo/bar.ts",
    sourceCode: fooBarTsSource,
    outputCode: fooBarTsOutput
  });
});

test(function compilerRunMultiModule() {
  // equal to `deno foo/baz.ts`
  reset();
  let factoryRun = false;
  mockDeps = ["require", "exports", "compiler"];
  mockFactory = (...deps: any[]) => {
    const [_require, _exports, _compiler] = deps;
    assertEqual(typeof _require, "function");
    assertEqual(typeof _exports, "object");
    assertEqual(_compiler, compiler);
    factoryRun = true;
    Object.defineProperty(_exports, "__esModule", { value: true });
    _exports.foo = "bar";
    // it is too complicated to test the outer factory, because the localised
    // make define already has a reference to this factory and it can't really
    // be easily unwound.  So we will do what we can with the inner one and
    // then just clear it...
    mockDeps = undefined;
    mockFactory = undefined;
  };

  const result = compilerInstance.run("foo/baz.ts", "/root/project");
  assert(result instanceof ModuleMetaData);
  // we have mocked that foo/bar.ts is already cached, so two fetches,
  // but only a single cache
  assertEqual(codeFetchStack.length, 2);
  assertEqual(codeCacheStack.length, 1);
  // because of the challenges with the way the module factories are generated
  // we only get one invocation of the `globalEval` mock.
  assertEqual(globalEvalStack.length, 1);
  assert(factoryRun);
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
  compilerInstance.run("foo/bar.ts", "/root/project");
  const result = compilerInstance.getScriptFileNames();
  assertEqual(result.length, 1, "Expected only a single filename.");
  assertEqual(result[0], "/root/project/foo/bar.ts");
});

test(function compilerGetScriptKind() {
  assertEqual(compilerInstance.getScriptKind("foo.ts"), ScriptKind.TS);
  assertEqual(compilerInstance.getScriptKind("foo.d.ts"), ScriptKind.TS);
  assertEqual(compilerInstance.getScriptKind("foo.js"), ScriptKind.JS);
  assertEqual(compilerInstance.getScriptKind("foo.json"), ScriptKind.JSON);
  assertEqual(compilerInstance.getScriptKind("foo.txt"), ScriptKind.JS);
});

test(function compilerGetScriptVersion() {
  const moduleMetaData = compilerInstance.resolveModule(
    "foo/bar.ts",
    "/root/project"
  );
  assertEqual(
    compilerInstance.getScriptVersion(moduleMetaData.fileName),
    "1",
    "Expected known module to have script version of 1"
  );
});

test(function compilerGetScriptVersionUnknown() {
  assertEqual(
    compilerInstance.getScriptVersion("/root/project/unknown_module.ts"),
    "",
    "Expected unknown module to have an empty script version"
  );
});

test(function compilerGetScriptSnapshot() {
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
  assert(!("dispose" in result));
});

test(function compilerGetCurrentDirectory() {
  assertEqual(compilerInstance.getCurrentDirectory(), "");
});

test(function compilerGetDefaultLibFileName() {
  assertEqual(
    compilerInstance.getDefaultLibFileName(),
    "$asset$/lib.globals.d.ts"
  );
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
  const moduleMetaData = compilerInstance.resolveModule(
    "foo/bar.ts",
    "/root/project"
  );
  assert(compilerInstance.fileExists(moduleMetaData.fileName));
  assert(compilerInstance.fileExists("$asset$/compiler.d.ts"));
  assertEqual(
    compilerInstance.fileExists("/root/project/unknown-module.ts"),
    false
  );
});

test(function compilerResolveModuleNames() {
  const results = compilerInstance.resolveModuleNames(
    ["foo/bar.ts", "foo/baz.ts", "$asset$/lib.globals.d.ts", "deno"],
    "/root/project"
  );
  assertEqual(results.length, 4);
  const fixtures: Array<[string, boolean]> = [
    ["/root/project/foo/bar.ts", false],
    ["/root/project/foo/baz.ts", false],
    ["$asset$/lib.globals.d.ts", true],
    ["$asset$/deno.d.ts", true]
  ];
  for (let i = 0; i < results.length; i++) {
    const result = results[i];
    const [resolvedFileName, isExternalLibraryImport] = fixtures[i];
    assertEqual(result.resolvedFileName, resolvedFileName);
    assertEqual(result.isExternalLibraryImport, isExternalLibraryImport);
  }
});

// Remove the mocks
test(function compilerTestsTeardown() {
  Object.assign(compilerInstance, originals);
});
