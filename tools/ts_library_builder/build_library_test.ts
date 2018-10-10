// Run this manually with:
//
//  ts-node tools/ts_library_builder/build_library_test.ts
import { test, assertEqual } from "../../js/testing/testing";
import { merge } from "./build_library";
import { Project, SourceFile } from "ts-simple-ast";

test(function simple() {
  console.log("hello world");
  assertEqual(false, true);
});

test(function buildLibraryMerge() {
  const inputProject = new Project();
  const declarationProject = new Project();
  const targetSourceFile = new SourceFile();
  merge({
    basePath: "tools/ts_library_builder/testdata",
    declarationProject,
    debug: true,
    globalVarName: "global",
    filePath: ".",
    inputProject,
    interfaceName: "Window",
    namespaceName: `"deno"`,
    targetSourceFile,
  });
});
