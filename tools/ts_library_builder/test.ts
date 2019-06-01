// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
// Run this manually with:
//
//  ./node_modules/.bin/ts-node --project tools/ts_library_builder/tsconfig.json tools/ts_library_builder/test.ts

import * as assert from "assert";
import { Project, ts } from "ts-morph";
import { flatten, mergeGlobals, prepareFileForMerge } from "./build_library";
import { inlineFiles, loadDtsFiles } from "./ast_util";

const { ModuleKind, ModuleResolutionKind, ScriptTarget } = ts;

/** setups and returns the fixtures for testing */
// eslint-disable-next-line @typescript-eslint/explicit-function-return-type
function setupFixtures() {
  const basePath = process.cwd();
  const buildPath = `${basePath}/tools/ts_library_builder/testdata`;
  const outputFile = `${buildPath}/lib.output.d.ts`;
  const inputProject = new Project({
    compilerOptions: {
      baseUrl: basePath,
      declaration: true,
      emitDeclarationOnly: true,
      module: ModuleKind.ESNext,
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
  loadDtsFiles(declarationProject, {});
  for (const { filePath, text } of inputProject.emitToMemory().getFiles()) {
    declarationProject.createSourceFile(filePath, text);
  }
  const outputProject = new Project({
    compilerOptions: {},
    useVirtualFileSystem: true
  });
  loadDtsFiles(outputProject, {});
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

function buildLibraryFlatten(): void {
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
    moduleName: `"api"`,
    namespaceName: "Api",
    targetSourceFile
  });

  assert(targetSourceFile.getNamespace(`"api"`) != null);
  assert(targetSourceFile.getNamespace("Api") != null);
  assert.equal(targetSourceFile.getNamespaces().length, 2);
  const moduleApi = targetSourceFile.getNamespaceOrThrow(`"api"`);
  const functions = moduleApi.getFunctions();
  assert.equal(functions[0].getName(), "foo");
  assert.equal(
    functions[0]
      .getJsDocs()
      .map(jsdoc => jsdoc.getInnerText())
      .join("\n"),
    "jsdoc for foo"
  );
  assert.equal(functions[1].getName(), "bar");
  assert.equal(
    functions[1]
      .getJsDocs()
      .map(jsdoc => jsdoc.getInnerText())
      .join("\n"),
    ""
  );
  assert.equal(functions.length, 2);
  const classes = moduleApi.getClasses();
  assert.equal(classes[0].getName(), "Foo");
  assert.equal(classes.length, 1);
  const variableDeclarations = moduleApi.getVariableDeclarations();
  assert.equal(variableDeclarations[0].getName(), "arr");
  assert.equal(variableDeclarations.length, 1);

  const namespaceApi = targetSourceFile.getNamespaceOrThrow(`"api"`);
  const functionsNs = namespaceApi.getFunctions();
  assert.equal(functionsNs[0].getName(), "foo");
  assert.equal(
    functionsNs[0]
      .getJsDocs()
      .map(jsdoc => jsdoc.getInnerText())
      .join("\n"),
    "jsdoc for foo"
  );
  assert.equal(functionsNs[1].getName(), "bar");
  assert.equal(
    functionsNs[1]
      .getJsDocs()
      .map(jsdoc => jsdoc.getInnerText())
      .join("\n"),
    ""
  );
  assert.equal(functionsNs.length, 2);
  const classesNs = namespaceApi.getClasses();
  assert.equal(classesNs[0].getName(), "Foo");
  assert.equal(classesNs.length, 1);
  const variableDeclarationsNs = namespaceApi.getVariableDeclarations();
  assert.equal(variableDeclarationsNs[0].getName(), "arr");
  assert.equal(variableDeclarationsNs.length, 1);
}

function buildLibraryMerge(): void {
  const {
    basePath,
    buildPath,
    declarationProject,
    debug,
    inputProject,
    outputSourceFile: targetSourceFile
  } = setupFixtures();

  const prepareForMergeOpts = {
    globalVarName: "foobarbaz",
    interfaceName: "FooBar",
    targetSourceFile
  };

  const prepareReturn = prepareFileForMerge(prepareForMergeOpts);

  mergeGlobals({
    basePath,
    declarationProject,
    debug,
    filePath: `${buildPath}/globals.ts`,
    inputProject,
    ...prepareForMergeOpts,
    prepareReturn
  });

  assert(targetSourceFile.getNamespace("moduleC") != null);
  assert(targetSourceFile.getNamespace("moduleD") != null);
  assert(targetSourceFile.getNamespace("moduleE") != null);
  assert(targetSourceFile.getNamespace("moduleF") != null);
  assert.equal(targetSourceFile.getNamespaces().length, 4);
  assert(targetSourceFile.getInterface("FooBar") != null);
  assert.equal(targetSourceFile.getInterfaces().length, 2);
  const variableDeclarations = targetSourceFile.getVariableDeclarations();
  assert.equal(variableDeclarations[0].getType().getText(), `FooBar`);
  assert.equal(variableDeclarations[1].getType().getText(), `moduleC.Bar`);
  assert.equal(
    variableDeclarations[2].getType().getText(),
    `typeof moduleC.qat`
  );
  assert.equal(
    variableDeclarations[3].getType().getText(),
    `typeof moduleE.process`
  );
  assert.equal(
    variableDeclarations[4].getType().getText(),
    `typeof moduleD.reprocess`
  );
  assert.equal(
    variableDeclarations[5].getType().getText(),
    `typeof moduleC.Bar`
  );
  assert.equal(variableDeclarations.length, 6);
  const typeAliases = targetSourceFile.getTypeAliases();
  assert.equal(typeAliases[0].getName(), "Bar");
  assert.equal(typeAliases[0].getType().getText(), "moduleC.Bar");
  assert.equal(typeAliases.length, 1);
  const exportedInterface = targetSourceFile.getInterfaceOrThrow("FizzBuzz");
  const interfaceProperties = exportedInterface.getStructure().properties;
  assert(interfaceProperties != null);
  assert.equal(interfaceProperties!.length, 2);
  assert.equal(interfaceProperties![0].name, "foo");
  assert.equal(interfaceProperties![0].type, "string");
  assert.equal(interfaceProperties![1].name, "bar");
  assert.equal(interfaceProperties![1].type, "number");
}

function testInlineFiles(): void {
  const {
    basePath,
    buildPath,
    debug,
    outputSourceFile: targetSourceFile
  } = setupFixtures();

  inlineFiles({
    basePath,
    debug,
    inline: [`${buildPath}/lib.extra.d.ts`],
    targetSourceFile
  });

  assert(targetSourceFile.getNamespace("Qat") != null);
  const qatNamespace = targetSourceFile.getNamespaceOrThrow("Qat");
  assert(qatNamespace.getClass("Foo") != null);
}

// TODO author unit tests for `ast_util.ts`

function main(): void {
  console.log("ts_library_builder buildLibraryFlatten");
  buildLibraryFlatten();
  console.log("ts_library_builder buildLibraryMerge");
  buildLibraryMerge();
  console.log("ts_library_builder testInlineFiles");
  testInlineFiles();
  console.log("ts_library_builder ok");
}

main();
