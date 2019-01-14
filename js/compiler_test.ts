// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { test, assert, assertEqual } from "./test_util.ts";
import * as deno from "deno";

// We use a silly amount of `any` in these tests...
// tslint:disable:no-any

const { Compiler, jsonEsmTemplate } = (deno as any)._compiler;

interface ModuleInfo {
  moduleName: string | undefined;
  filename: string | undefined;
  mediaType: MediaType | undefined;
  sourceCode: string | undefined;
  outputCode: string | undefined;
  sourceMap: string | undefined;
}

// Since we can't/don't want to import all of TypeScript for this one enum, we
// we will replicate it from TypeScript.  This does mean that if other script
// kinds are added in the future we would need to add them manually to the tests
enum ScriptKind {
  Unknown = 0,
  JS = 1,
  JSX = 2,
  TS = 3,
  TSX = 4,
  External = 5,
  JSON = 6,
  Deferred = 7
}

const compilerInstance = Compiler.instance();

// References to original items we are going to mock
const originals = {
  _log: (compilerInstance as any)._log,
  _os: (compilerInstance as any)._os,
  _ts: (compilerInstance as any)._ts,
  _service: (compilerInstance as any)._service
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

// Some fixtures we will use in testing
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
const fooBarTsOutput = `import * as deno from "deno";
console.log(deno);
export const foo = "bar";
//# sourceMappingURL=bar.js.map
//# sourceURL=/root/project/foo/bar.ts`;

const fooBarTsSourcemap = `{"version":3,"file":"bar.js","sourceRoot":"","sources":["file:///root/project/foo/bar.ts"],"names":[],"mappings":"AAAA,OAAO,KAAK,IAAI,MAAM,MAAM,CAAC;AAC7B,OAAO,CAAC,GAAG,CAAC,IAAI,CAAC,CAAC;AAClB,MAAM,CAAC,MAAM,GAAG,GAAG,KAAK,CAAC"}`;

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
      null,
      null
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
    ),
    "empty_file.ts": mockModuleInfo(
      "/moduleKinds/empty_file.ts",
      "/moduleKinds/empty_file.ts",
      MediaType.TypeScript,
      "",
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
  createLanguageService() {
    return {} as any;
  },
  formatDiagnosticsWithColorAndContext(
    diagnostics: ReadonlyArray<any>,
    _host: any
  ): string {
    return JSON.stringify(diagnostics.map(({ messageText }) => messageText));
  }
};

const serviceMock = {
  getCompilerOptionsDiagnostics() {
    return originals._service.getCompilerOptionsDiagnostics.call(
      originals._service
    );
  },
  getEmitOutput(fileName: string) {
    getEmitOutputStack.push(fileName);
    return originals._service.getEmitOutput.call(originals._service, fileName);
  },
  getSemanticDiagnostics(fileName: string) {
    return originals._service.getSemanticDiagnostics.call(
      originals._service,
      fileName
    );
  },
  getSyntacticDiagnostics(fileName: string) {
    return originals._service.getSyntacticDiagnostics.call(
      originals._service,
      fileName
    );
  }
};
const mocks = {
  _log: logMock,
  _os: osMock,
  _ts: tsMock,
  _service: serviceMock
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

  assertEqual(mockDepsStack.length, 0);
  assertEqual(mockFactoryStack.length, 0);
  mockDepsStack = [];
  mockFactoryStack = [];

  // restore original properties and methods
  Object.assign(compilerInstance, originals);
}

test(function testJsonEsmTemplate() {
  const result = jsonEsmTemplate(
    `{ "hello": "world", "foo": "bar" }`,
    "/foo.ts"
  );
  assertEqual(
    result,
    `const _json = JSON.parse(\`{ "hello": "world", "foo": "bar" }\`)\n` +
      `export default _json;\n` +
      `//# sourceURL=/foo.ts`
  );
});

test(function compilerInstance() {
  assert(Compiler != null);
  assert(Compiler.instance() != null);
});

// Testing the internal APIs

test(function compilerCompile() {
  // equal to `deno foo/bar.ts`
  setup();
  const moduleMetaData = compilerInstance.compile(
    "foo/bar.ts",
    "/root/project"
  );
  assertEqual(moduleMetaData.sourceCode, fooBarTsSource);
  assertEqual(moduleMetaData.outputCode, fooBarTsOutput);

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

test(function compilerCompilerMultiModule() {
  // equal to `deno foo/baz.ts`
  setup();
  compilerInstance.compile("foo/baz.ts", "/root/project");
  assertEqual(codeFetchStack.length, 2, "Two modules fetched.");
  assertEqual(codeCacheStack.length, 1, "Only one module compiled.");
  teardown();
});

test(function compilerLoadJsonModule() {
  setup();
  compilerInstance.compile("loadConfig.ts", "/root/project");
  assertEqual(codeFetchStack.length, 2, "Two modules fetched.");
  assertEqual(codeCacheStack.length, 1, "Only one module compiled.");
  teardown();
});

test(function compilerResolveModule() {
  setup();
  const moduleMetaData = compilerInstance.resolveModule(
    "foo/baz.ts",
    "/root/project"
  );
  console.log(moduleMetaData);
  assertEqual(moduleMetaData.sourceCode, fooBazTsSource);
  assertEqual(moduleMetaData.outputCode, null);
  assertEqual(moduleMetaData.sourceMap, null);
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

test(function compilerRecompileFlag() {
  setup();
  compilerInstance.compile("foo/bar.ts", "/root/project");
  assertEqual(
    getEmitOutputStack.length,
    1,
    "Expected only a single emitted file."
  );
  // running compiler against same file should use cached code
  compilerInstance.compile("foo/bar.ts", "/root/project");
  assertEqual(
    getEmitOutputStack.length,
    1,
    "Expected only a single emitted file."
  );
  compilerInstance.recompile = true;
  compilerInstance.compile("foo/bar.ts", "/root/project");
  assertEqual(getEmitOutputStack.length, 2, "Expected two emitted file.");
  assert(
    getEmitOutputStack[0] === getEmitOutputStack[1],
    "Expected same file to be emitted twice."
  );
  teardown();
});

// TypeScript LanguageServiceHost APIs

test(function compilerGetCompilationSettings() {
  const expectedKeys = [
    "allowJs",
    "checkJs",
    "esModuleInterop",
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
  compilerInstance.compile("foo/bar.ts", "/root/project");
  const result = compilerInstance.getScriptFileNames();
  assertEqual(result.length, 1, "Expected only a single filename.");
  assertEqual(result[0], "/root/project/foo/bar.ts");
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
    ScriptKind.TS
  );
  assertEqual(
    compilerInstance.getScriptKind("/moduleKinds/foo.d.ts"),
    ScriptKind.TS
  );
  assertEqual(
    compilerInstance.getScriptKind("/moduleKinds/foo.js"),
    ScriptKind.JS
  );
  assertEqual(
    compilerInstance.getScriptKind("/moduleKinds/foo.json"),
    ScriptKind.JSON
  );
  assertEqual(
    compilerInstance.getScriptKind("/moduleKinds/foo.txt"),
    ScriptKind.JS
  );
  teardown();
});

test(function compilerGetScriptVersion() {
  setup();
  const moduleMetaData = compilerInstance.compile(
    "foo/bar.ts",
    "/root/project"
  );
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

test(function compilerResolveEmptyFile() {
  setup();
  const result = compilerInstance.resolveModuleNames(
    ["empty_file.ts"],
    "/moduleKinds"
  );
  assertEqual(result[0].resolvedFileName, "/moduleKinds/empty_file.ts");
  teardown();
});
