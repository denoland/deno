import { writeFileSync } from "fs";
import { join } from "path";
import * as prettier from "prettier";
import {
  InterfaceDeclaration,
  ExpressionStatement,
  NamespaceDeclarationKind,
  Project,
  SourceFile,
  ts,
  Type,
  TypeGuards
} from "ts-morph";
import {
  addInterfaceDeclaration,
  addInterfaceProperty,
  addSourceComment,
  addTypeAlias,
  addVariableDeclaration,
  checkDiagnostics,
  flattenNamespace,
  getSourceComment,
  inlineFiles,
  loadDtsFiles,
  loadFiles,
  log,
  logDiagnostics,
  namespaceSourceFile,
  normalizeSlashes,
  setSilent
} from "./ast_util";

export interface BuildLibraryOptions {
  /**
   * The path to the root of the deno repository
   */
  basePath: string;

  /**
   * The path to the current build path
   */
  buildPath: string;

  /**
   * Denotes if the library should be built with debug information (comments
   * that indicate the source of the types)
   */
  debug?: boolean;

  /**
   * An array of files that should be inlined into the library
   */
  inline?: string[];

  /** An array of input files to be provided to the input project, relative to
   * the basePath. */
  inputs?: string[];

  /**
   * Path to globals file to be used I.E. `js/globals.ts`
   */
  additionalGlobals?: string[];

  /**
   * List of global variables to define as let instead of the default const.
   */
  declareAsLet?: string[];

  /**
   * The path to the output library
   */
  outFile: string;

  /**
   * Execute in silent mode or not
   */
  silent?: boolean;
}

const { ModuleKind, ModuleResolutionKind, ScriptTarget } = ts;

/**
 * A preamble which is appended to the start of the library.
 */
const libPreamble = `// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.

/// <reference no-default-lib="true" />
/// <reference lib="esnext" />

`;

// The path to the msg_generated file relative to the build path
const MSG_GENERATED_PATH = "/gen/cli/msg_generated.ts";

// An array of enums we want to expose pub
const MSG_GENERATED_ENUMS = ["ErrorKind"];

/** Extracts enums from a source file */
function extract(sourceFile: SourceFile, enumNames: string[]): string {
  // Copy specified enums from msg_generated
  let output = "";
  for (const enumName of enumNames) {
    const enumDeclaration = sourceFile.getEnumOrThrow(enumName);
    enumDeclaration.setHasDeclareKeyword(false);
    // we are not copying JSDocs or other trivia here because msg_generated only
    // contains some non-useful JSDocs and comments that are not ideal to copy
    // over
    output += enumDeclaration.getText();
  }
  return output;
}

interface FlattenOptions {
  basePath: string;
  customSources: { [filePath: string]: string };
  filePath: string;
  debug?: boolean;
  declarationProject: Project;
  globalInterfaceName?: string;
  moduleName?: string;
  namespaceName?: string;
  targetSourceFile: SourceFile;
}

/** Flatten a module */
export function flatten({
  basePath,
  customSources,
  filePath,
  debug,
  declarationProject,
  globalInterfaceName,
  moduleName,
  namespaceName,
  targetSourceFile
}: FlattenOptions): void {
  // Flatten the source file into a single set of statements
  const statements = flattenNamespace({
    sourceFile: declarationProject.getSourceFileOrThrow(filePath),
    rootPath: basePath,
    customSources,
    debug
  });

  // If a module name is specified create the module in the target file
  if (moduleName) {
    const namespace = targetSourceFile.addNamespace({
      name: moduleName,
      hasDeclareKeyword: true,
      declarationKind: NamespaceDeclarationKind.Module
    });

    // Add the output of the flattening to the namespace
    namespace.addStatements(statements);
  }

  if (namespaceName) {
    const namespace = targetSourceFile.insertNamespace(0, {
      name: namespaceName,
      hasDeclareKeyword: true,
      declarationKind: NamespaceDeclarationKind.Namespace
    });

    // Add the output of the flattening to the namespace
    namespace.addStatements(statements);

    if (globalInterfaceName) {
      // Retrieve the global interface
      const interfaceDeclaration = targetSourceFile.getInterfaceOrThrow(
        globalInterfaceName
      );

      // Add the namespace to the global interface
      addInterfaceProperty(
        interfaceDeclaration,
        namespaceName,
        `typeof ${namespaceName}`
      );
    }
  }
}

interface PrepareFileForMergeOptions {
  globalVarName: string;
  interfaceName: string;
  targetSourceFile: SourceFile;
}

interface PrepareFileForMergeReturn {
  interfaceDeclaration: InterfaceDeclaration;
}

export function prepareFileForMerge({
  globalVarName,
  interfaceName,
  targetSourceFile
}: PrepareFileForMergeOptions): PrepareFileForMergeReturn {
  // Add the global object interface
  const interfaceDeclaration = targetSourceFile.addInterface({
    name: interfaceName,
    hasDeclareKeyword: true
  });

  // Declare the global variable
  addVariableDeclaration(
    targetSourceFile,
    globalVarName,
    interfaceName,
    true,
    true
  );

  // Add self reference to the global variable
  addInterfaceProperty(interfaceDeclaration, globalVarName, interfaceName);

  return {
    interfaceDeclaration
  };
}

interface MergeGlobalOptions extends PrepareFileForMergeOptions {
  basePath: string;
  debug?: boolean;
  declarationProject: Project;
  filePath: string;
  ignore?: string[];
  inputProject: Project;
  prepareReturn: PrepareFileForMergeReturn;
  declareAsLet?: string[];
}

/** Take a module and merge it into the global scope */
export function mergeGlobals({
  basePath,
  debug,
  declarationProject,
  filePath,
  globalVarName,
  ignore,
  inputProject,
  targetSourceFile,
  declareAsLet,
  prepareReturn: { interfaceDeclaration }
}: MergeGlobalOptions): void {
  // Retrieve source file from the input project
  const sourceFile = inputProject.getSourceFileOrThrow(filePath);

  // we are going to create a map of variables
  const globalVariables = new Map<
    string,
    {
      type: Type;
      node: ExpressionStatement;
    }
  >();
  const globalInterfaces: InterfaceDeclaration[] = [];

  // For every augmentation of the global variable in source file, we want
  // to extract the type and add it to the global variable map
  sourceFile.forEachChild(node => {
    if (TypeGuards.isExpressionStatement(node)) {
      const firstChild = node.getFirstChild();
      if (!firstChild) {
        return;
      }
      if (TypeGuards.isBinaryExpression(firstChild)) {
        const leftExpression = firstChild.getLeft();
        if (
          TypeGuards.isPropertyAccessExpression(leftExpression) &&
          leftExpression.getExpression().getText() === globalVarName
        ) {
          const globalVarProperty = leftExpression.getName();
          if (globalVarProperty !== globalVarName) {
            globalVariables.set(globalVarProperty, {
              type: firstChild.getType(),
              node
            });
          }
        }
      }
    } else if (TypeGuards.isInterfaceDeclaration(node) && node.isExported()) {
      globalInterfaces.push(node);
    }
  });

  // A set of source files that the types we are using are dependent on us
  // importing
  const dependentSourceFiles = new Set<SourceFile>();

  // Create a global variable and add the property to the `Window` interface
  // for each mutation of the `window` variable we observed in `globals.ts`
  for (const [property, info] of globalVariables) {
    if (!(ignore && ignore.includes(property))) {
      const type = info.type.getText(info.node);
      const typeSymbol = info.type.getSymbol();
      if (typeSymbol) {
        const valueDeclaration = typeSymbol.getValueDeclaration();
        if (valueDeclaration) {
          dependentSourceFiles.add(valueDeclaration.getSourceFile());
        }
      }
      const isConst = !(declareAsLet && declareAsLet.includes(property));
      addVariableDeclaration(targetSourceFile, property, type, isConst, true);
      addInterfaceProperty(interfaceDeclaration, property, type);
    }
  }

  // We need to copy over any type aliases
  for (const typeAlias of sourceFile.getTypeAliases()) {
    addTypeAlias(
      targetSourceFile,
      typeAlias.getName(),
      typeAlias.getType().getText(sourceFile),
      true
    );
  }

  // We need to copy over any interfaces
  for (const interfaceDeclaration of globalInterfaces) {
    addInterfaceDeclaration(targetSourceFile, interfaceDeclaration);
  }

  // We need to ensure that we only namespace each source file once, so we
  // will use this map for tracking that.
  const sourceFileMap = new Map<SourceFile, string>();

  // For each import declaration in source file we will want to convert the
  // declaration source file into a namespace that exists within the merged
  // namespace
  const importDeclarations = sourceFile.getImportDeclarations();
  const namespaces = new Set<string>();
  for (const declaration of importDeclarations) {
    const namespaceImport = declaration.getNamespaceImport();
    if (namespaceImport) {
      const declarationSourceFile = declaration.getModuleSpecifierSourceFile();
      if (
        declarationSourceFile &&
        dependentSourceFiles.has(declarationSourceFile)
      ) {
        // the source file will resolve to the original `.ts` file, but the
        // information we really want is in the emitted `.d.ts` file, so we will
        // resolve to that file
        const dtsFilePath = declarationSourceFile
          .getFilePath()
          .replace(/\.ts$/, ".d.ts");
        const dtsSourceFile = declarationProject.getSourceFileOrThrow(
          dtsFilePath
        );
        targetSourceFile.addStatements(
          namespaceSourceFile(dtsSourceFile, {
            debug,
            namespace: namespaceImport.getText(),
            namespaces,
            rootPath: basePath,
            sourceFileMap
          })
        );
      }
    }
  }

  if (debug) {
    addSourceComment(targetSourceFile, sourceFile, basePath);
  }
}

/**
 * Generate the runtime library for Deno and write it to the supplied out file
 * name.
 */
export function main({
  basePath,
  buildPath,
  inline,
  inputs,
  additionalGlobals,
  declareAsLet,
  debug,
  outFile,
  silent
}: BuildLibraryOptions): void {
  setSilent(silent);
  log("-----");
  log("build_lib");
  log();
  log(`basePath: "${basePath}"`);
  log(`buildPath: "${buildPath}"`);
  if (inline && inline.length) {
    log("inline:");
    for (const filename of inline) {
      log(`  "${filename}"`);
    }
  }
  if (inputs && inputs.length) {
    log("inputs:");
    for (const input of inputs) {
      log(`  "${input}"`);
    }
  }
  log(`debug: ${!!debug}`);
  log(`outFile: "${outFile}"`);
  log();

  // the inputProject will take in the TypeScript files that are internal
  // to Deno to be used to generate the library
  const inputProject = new Project({
    compilerOptions: {
      baseUrl: basePath,
      declaration: true,
      emitDeclarationOnly: true,
      lib: ["esnext"],
      module: ModuleKind.ESNext,
      moduleResolution: ModuleResolutionKind.NodeJs,
      paths: {
        "*": ["*", `${buildPath}/*`]
      },
      preserveConstEnums: true,
      strict: true,
      stripInternal: true,
      target: ScriptTarget.ESNext
    }
  });

  // Add the input files we will need to generate the declarations, `globals`
  // plus any modules that are importable in the runtime need to be added here
  // plus the `lib.esnext` which is used as the base library
  if (inputs) {
    inputProject.addExistingSourceFiles(
      inputs.map(input => join(basePath, input))
    );
  }

  // emit the project, which will be only the declaration files
  const inputEmitResult = inputProject.emitToMemory();

  log("Emitted input project.");

  const inputDiagnostics = inputEmitResult
    .getDiagnostics()
    .map(d => d.compilerObject);
  logDiagnostics(inputDiagnostics);
  if (inputDiagnostics.length) {
    console.error("\nDiagnostics present during input project emit.\n");
    process.exit(1);
  }

  // the declaration project will be the target for the emitted files from
  // the input project, these will be used to transfer information over to
  // the final library file
  const declarationProject = new Project({
    compilerOptions: {
      baseUrl: basePath,
      lib: ["esnext"],
      moduleResolution: ModuleResolutionKind.NodeJs,
      paths: {
        "*": ["*", `${buildPath}/*`]
      },
      strict: true,
      target: ScriptTarget.ESNext
    },
    useVirtualFileSystem: true
  });

  // we don't want to add to the declaration project any of the original
  // `.ts` source files, so we need to filter those out
  const jsPath = normalizeSlashes(`${basePath}/js`);
  const inputProjectFiles = inputProject
    .getSourceFiles()
    .map(sourceFile => sourceFile.getFilePath())
    .filter(filePath => !filePath.startsWith(jsPath));
  loadFiles(declarationProject, inputProjectFiles);

  // now we add the emitted declaration files from the input project
  for (const { filePath, text } of inputEmitResult.getFiles()) {
    declarationProject.createSourceFile(filePath, text);
  }

  // the outputProject will contain the final library file we are looking to
  // build
  const outputProjectCompilerOptions: ts.CompilerOptions = {
    baseUrl: buildPath,
    lib: ["esnext"],
    moduleResolution: ModuleResolutionKind.NodeJs,
    strict: true,
    target: ScriptTarget.ESNext
  };

  const outputProject = new Project({
    compilerOptions: outputProjectCompilerOptions,
    useVirtualFileSystem: true
  });

  // There are files we need to load into memory, so that the project "compiles"
  loadDtsFiles(outputProject, outputProjectCompilerOptions);

  // libDts is the final output file we are looking to build and we are not
  // actually creating it, only in memory at this stage.
  const libDTs = outputProject.createSourceFile(outFile);

  // Deal with `js/deno.ts`

  // `gen/msg_generated.d.ts` contains too much exported information that is not
  // part of the public API surface of Deno, so we are going to extract just the
  // information we need.
  const msgGeneratedDts = inputProject.getSourceFileOrThrow(
    `${buildPath}${MSG_GENERATED_PATH}`
  );
  const msgGeneratedDtsText = extract(msgGeneratedDts, MSG_GENERATED_ENUMS);

  // Generate a object hash of substitutions of modules to use when flattening
  const customSources = {
    [msgGeneratedDts.getFilePath().replace(/(\.d)?\.ts$/, "")]: `${
      debug ? getSourceComment(msgGeneratedDts, basePath) : ""
    }${msgGeneratedDtsText}\n`
  };

  const prepareForMergeOpts: PrepareFileForMergeOptions = {
    globalVarName: "window",
    interfaceName: "Window",
    targetSourceFile: libDTs
  };

  const prepareReturn = prepareFileForMerge(prepareForMergeOpts);

  mergeGlobals({
    basePath,
    debug,
    declarationProject,
    filePath: `${basePath}/js/globals.ts`,
    inputProject,
    ignore: ["Deno"],
    declareAsLet,
    ...prepareForMergeOpts,
    prepareReturn
  });

  log(`Merged "globals" into global scope.`);

  if (additionalGlobals) {
    for (const additionalGlobal of additionalGlobals) {
      mergeGlobals({
        basePath,
        debug,
        declarationProject,
        filePath: `${basePath}/${additionalGlobal}`,
        inputProject,
        ignore: ["Deno"],
        declareAsLet,
        ...prepareForMergeOpts,
        prepareReturn
      });
    }

    log(`Added additional "globals" into global scope.`);
  }

  flatten({
    basePath,
    customSources,
    debug,
    declarationProject,
    filePath: `${basePath}/js/deno.d.ts`,
    globalInterfaceName: "Window",
    namespaceName: "Deno",
    targetSourceFile: libDTs
  });

  log(`Created module "deno" and namespace Deno.`);

  // Inline any files that were passed in, to be used to add additional libs
  // which are not part of TypeScript.
  if (inline && inline.length) {
    inlineFiles({
      basePath,
      debug,
      inline,
      targetSourceFile: libDTs
    });
  }

  // Add the preamble
  libDTs.insertStatements(0, libPreamble);

  // Check diagnostics
  checkDiagnostics(outputProject);

  // Output the final library file
  libDTs.saveSync();
  const libDTsText = prettier.format(
    outputProject.getFileSystem().readFileSync(outFile, "utf8"),
    { parser: "typescript" }
  );
  if (!silent) {
    console.log(`Outputting library to: "${outFile}"`);
    console.log(`  Length: ${libDTsText.length}`);
  }
  writeFileSync(outFile, libDTsText, { encoding: "utf8" });
  if (!silent) {
    console.log("-----");
    console.log();
  }
}
