// Run this manually with:
//
//  ./node_modules/.bin/ts-node --project tools/ts_library_builder/tsconfig.json tools/ts_library_builder/test.ts

import { Project, ts } from "ts-simple-ast";
import { assert, assertEqual, test } from "../../js/testing/testing";
import { flatten, merge } from "./build_library";
import { loadDtsFiles } from "./ast_util";

const { ModuleKind, ModuleResolutionKind, ScriptTarget } = ts;

/** setups and returns the fixtures for testing */
function setupFixtures() {
  const basePath = process.cwd();
  const buildPath = `${basePath}/tools/ts_library_builder/testdata`;
  const outputFile = `${buildPath}/lib.output.d.ts`;
  const inputProject = new Project({
    compilerOptions: {
      baseUrl: basePath,
      declaration: true,
      emitDeclarationOnly: true,
      module: ModuleKind.AMD,
      moduleResolution: ModuleResolutionKind.NodeJs,
      strict: true,
      stripInternal: true,
      target: ScriptTarget.ESNext
    }
  });
  inputProject.addExistingSourceFiles([
    `${buildPath}/globals.ts`,
    `${buildPath}/api.ts`
  ]);
  const declarationProject = new Project({
    compilerOptions: {},
    useVirtualFileSystem: true
  });
  loadDtsFiles(declarationProject);
  for (const { filePath, text } of inputProject.emitToMemory().getFiles()) {
    declarationProject.createSourceFile(filePath, text);
  }
  const outputProject = new Project({
    compilerOptions: {},
    useVirtualFileSystem: true
  });
  loadDtsFiles(outputProject);
  const outputSourceFile = outputProject.createSourceFile(outputFile);
  const debug = true;

  return {
    basePath,
    buildPath,
    inputProject,
    outputFile,
    declarationProject,
    outputProject,
    outputSourceFile,
    debug
  };
}

test(function buildLibraryFlatten() {
  const {
    basePath,
    buildPath,
    debug,
    declarationProject,
    outputSourceFile: targetSourceFile
  } = setupFixtures();

  flatten({
    basePath,
    customSources: {},
    debug,
    declarationProject,
    filePath: `${buildPath}/api.d.ts`,
    namespaceName: `"api"`,
    targetSourceFile
  });

  assert(targetSourceFile.getNamespace(`"api"`) != null);
  assertEqual(targetSourceFile.getNamespaces().length, 1);
  const namespaceApi = targetSourceFile.getNamespaceOrThrow(`"api"`);
  const functions = namespaceApi.getFunctions();
  assertEqual(functions[0].getName(), "foo");
  assertEqual(
    functions[0]
      .getJsDocs()
      .map(jsdoc => jsdoc.getInnerText())
      .join("\n"),
    "jsdoc for foo"
  );
  assertEqual(functions[1].getName(), "bar");
  assertEqual(
    functions[1]
      .getJsDocs()
      .map(jsdoc => jsdoc.getInnerText())
      .join("\n"),
    ""
  );
  assertEqual(functions.length, 2);
  const classes = namespaceApi.getClasses();
  assertEqual(classes[0].getName(), "Foo");
  assertEqual(classes.length, 1);
  const variableDeclarations = namespaceApi.getVariableDeclarations();
  assertEqual(variableDeclarations[0].getName(), "arr");
  assertEqual(variableDeclarations.length, 1);
});

test(function buildLibraryMerge() {
  const {
    basePath,
    buildPath,
    declarationProject,
    debug,
    inputProject,
    outputSourceFile: targetSourceFile
  } = setupFixtures();

  merge({
    basePath,
    declarationProject,
    debug,
    globalVarName: "foobarbaz",
    filePath: `${buildPath}/globals.ts`,
    inputProject,
    interfaceName: "FooBar",
    namespaceName: `"bazqat"`,
    targetSourceFile
  });

  assert(targetSourceFile.getNamespace(`"bazqat"`) != null);
  assertEqual(targetSourceFile.getNamespaces().length, 1);
  const namespaceBazqat = targetSourceFile.getNamespaceOrThrow(`"bazqat"`);
  assert(namespaceBazqat.getNamespace("global") != null);
  assert(namespaceBazqat.getNamespace("moduleC") != null);
  assertEqual(namespaceBazqat.getNamespaces().length, 2);
  assert(namespaceBazqat.getInterface("FooBar") != null);
  assertEqual(namespaceBazqat.getInterfaces().length, 1);
  const globalNamespace = namespaceBazqat.getNamespaceOrThrow("global");
  const variableDeclarations = globalNamespace.getVariableDeclarations();
  assertEqual(
    variableDeclarations[0].getType().getText(),
    `import("bazqat").FooBar`
  );
  assertEqual(
    variableDeclarations[1].getType().getText(),
    `import("bazqat").moduleC.Bar`
  );
  assertEqual(
    variableDeclarations[2].getType().getText(),
    `typeof import("bazqat").moduleC.qat`
  );
  assertEqual(variableDeclarations.length, 3);
});

// TODO author unit tests for `ast_util.ts`
