// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.
import { basename, dirname, join, relative } from "path";
import { readFileSync } from "fs";
import { EOL } from "os";
import {
  ExportDeclaration,
  ImportDeclaration,
  InterfaceDeclaration,
  JSDoc,
  Project,
  PropertySignature,
  SourceFile,
  StatementedNode,
  ts,
  TypeAliasDeclaration,
  TypeGuards,
  VariableStatement,
  VariableDeclarationKind
} from "ts-morph";

let silent = false;

/** Logs a message to the console. */
export function log(message: any = "", ...args: any[]): void {
  if (!silent) {
    console.log(message, ...args);
  }
}

/** Sets the silent flag which impacts logging to the console. */
export function setSilent(value = false): void {
  silent = value;
}

/** Add a property to an interface */
export function addInterfaceProperty(
  interfaceDeclaration: InterfaceDeclaration,
  name: string,
  type: string,
  jsdocs?: JSDoc[]
): PropertySignature {
  return interfaceDeclaration.addProperty({
    name,
    type,
    docs: jsdocs && jsdocs.map(jsdoc => jsdoc.getText())
  });
}

/** Add `@url` comment to node. */
export function addSourceComment(
  node: StatementedNode,
  sourceFile: SourceFile,
  rootPath: string
): void {
  node.insertStatements(
    0,
    `// @url ${relative(rootPath, sourceFile.getFilePath())}\n\n`
  );
}

/** Add a declaration of a type alias to a node */
export function addTypeAlias(
  node: StatementedNode,
  name: string,
  type: string,
  hasDeclareKeyword = false,
  jsdocs?: JSDoc[]
): TypeAliasDeclaration {
  return node.addTypeAlias({
    name,
    type,
    docs: jsdocs && jsdocs.map(jsdoc => jsdoc.getText()),
    hasDeclareKeyword
  });
}

/** Add a declaration of an interface to a node */
export function addInterfaceDeclaration(
  node: StatementedNode,
  interfaceDeclaration: InterfaceDeclaration
) {
  const interfaceStructure = interfaceDeclaration.getStructure();

  return node.addInterface({
    name: interfaceStructure.name,
    properties: interfaceStructure.properties,
    docs: interfaceStructure.docs,
    hasDeclareKeyword: true
  });
}

/** Add a declaration of a variable to a node */
export function addVariableDeclaration(
  node: StatementedNode,
  name: string,
  type: string,
  isConst: boolean,
  hasDeclareKeyword?: boolean,
  jsdocs?: JSDoc[]
): VariableStatement {
  return node.addVariableStatement({
    declarationKind: isConst
      ? VariableDeclarationKind.Const
      : VariableDeclarationKind.Let,
    declarations: [{ name, type }],
    docs: jsdocs && jsdocs.map(jsdoc => jsdoc.getText()),
    hasDeclareKeyword
  });
}

/** Copy one source file to the end of another source file. */
export function appendSourceFile(
  sourceFile: SourceFile,
  targetSourceFile: SourceFile
): void {
  targetSourceFile.addStatements(`\n${sourceFile.print()}`);
}

/** Used when formatting diagnostics */
const formatDiagnosticHost: ts.FormatDiagnosticsHost = {
  getCurrentDirectory() {
    return process.cwd();
  },
  getCanonicalFileName(path: string) {
    return path;
  },
  getNewLine() {
    return EOL;
  }
};

/** Log diagnostics to the console with colour. */
export function logDiagnostics(diagnostics: ts.Diagnostic[]): void {
  if (diagnostics.length) {
    console.log(
      ts.formatDiagnosticsWithColorAndContext(diagnostics, formatDiagnosticHost)
    );
  }
}

/** Check diagnostics, and if any exist, exit the process */
export function checkDiagnostics(project: Project, onlyFor?: string[]): void {
  const program = project.getProgram();
  const diagnostics = [
    ...program.getGlobalDiagnostics(),
    ...program.getSyntacticDiagnostics(),
    ...program.getSemanticDiagnostics(),
    ...program.getDeclarationDiagnostics()
  ]
    .filter(diagnostic => {
      const sourceFile = diagnostic.getSourceFile();
      return onlyFor && sourceFile
        ? onlyFor.includes(sourceFile.getFilePath())
        : true;
    })
    .map(diagnostic => diagnostic.compilerObject);

  logDiagnostics(diagnostics);

  if (diagnostics.length) {
    process.exit(1);
  }
}

function createDeclarationError(
  msg: string,
  declaration: ImportDeclaration | ExportDeclaration
): Error {
  return new Error(
    `${msg}\n` +
      `  In: "${declaration.getSourceFile().getFilePath()}"\n` +
      `  Text: "${declaration.getText()}"`
  );
}

export interface FlattenNamespaceOptions {
  customSources?: { [sourceFilePath: string]: string };
  debug?: boolean;
  rootPath: string;
  sourceFile: SourceFile;
}

/** Returns a string which indicates the source file as the source */
export function getSourceComment(
  sourceFile: SourceFile,
  rootPath: string
): string {
  return `\n// @url ${relative(rootPath, sourceFile.getFilePath())}\n\n`;
}

/** Return a set of fully qualified symbol names for the files exports */
function getExportedSymbols(sourceFile: SourceFile): Set<string> {
  const exportedSymbols = new Set<string>();
  const exportDeclarations = sourceFile.getExportDeclarations();
  for (const exportDeclaration of exportDeclarations) {
    const exportSpecifiers = exportDeclaration.getNamedExports();
    for (const exportSpecifier of exportSpecifiers) {
      const aliasedSymbol = exportSpecifier
        .getSymbolOrThrow()
        .getAliasedSymbol();
      if (aliasedSymbol) {
        exportedSymbols.add(aliasedSymbol.getFullyQualifiedName());
      }
    }
  }
  return exportedSymbols;
}

/** Take a namespace and flatten all exports. */
export function flattenNamespace({
  customSources,
  debug,
  rootPath,
  sourceFile
}: FlattenNamespaceOptions): string {
  const sourceFiles = new Set<SourceFile>();
  let output = "";
  const exportedSymbols = getExportedSymbols(sourceFile);

  function flattenDeclarations(
    declaration: ImportDeclaration | ExportDeclaration
  ): void {
    const declarationSourceFile = declaration.getModuleSpecifierSourceFile();
    if (declarationSourceFile) {
      // eslint-disable-next-line @typescript-eslint/no-use-before-define
      processSourceFile(declarationSourceFile);
      declaration.remove();
    }
  }

  function rectifyNodes(currentSourceFile: SourceFile): void {
    currentSourceFile.forEachChild(node => {
      if (TypeGuards.isAmbientableNode(node)) {
        node.setHasDeclareKeyword(false);
      }
      if (TypeGuards.isExportableNode(node)) {
        const nodeSymbol = node.getSymbol();
        if (
          nodeSymbol &&
          !exportedSymbols.has(nodeSymbol.getFullyQualifiedName())
        ) {
          node.setIsExported(false);
        }
      }
    });
  }

  function processSourceFile(
    currentSourceFile: SourceFile
  ): string | undefined {
    if (sourceFiles.has(currentSourceFile)) {
      return;
    }
    sourceFiles.add(currentSourceFile);

    const currentSourceFilePath = currentSourceFile
      .getFilePath()
      .replace(/(\.d)?\.ts$/, "");
    log("Process source file:", currentSourceFilePath);
    if (customSources && currentSourceFilePath in customSources) {
      log("  Using custom source.");
      output += customSources[currentSourceFilePath];
      return;
    }

    currentSourceFile.getImportDeclarations().forEach(flattenDeclarations);
    currentSourceFile.getExportDeclarations().forEach(flattenDeclarations);

    rectifyNodes(currentSourceFile);

    output +=
      (debug ? getSourceComment(currentSourceFile, rootPath) : "") +
      currentSourceFile.print();
  }

  sourceFile.getExportDeclarations().forEach(exportDeclaration => {
    const exportedSourceFile = exportDeclaration.getModuleSpecifierSourceFile();
    if (exportedSourceFile) {
      processSourceFile(exportedSourceFile);
    } else {
      throw createDeclarationError("Missing source file.", exportDeclaration);
    }
    exportDeclaration.remove();
  });

  rectifyNodes(sourceFile);

  return (
    output +
    (debug ? getSourceComment(sourceFile, rootPath) : "") +
    sourceFile.print()
  );
}

interface InlineFilesOptions {
  basePath: string;
  debug?: boolean;
  inline: string[];
  targetSourceFile: SourceFile;
}

/** Inline files into the target source file. */
export function inlineFiles({
  basePath,
  debug,
  inline,
  targetSourceFile
}: InlineFilesOptions): void {
  for (const filename of inline) {
    const text = readFileSync(filename, {
      encoding: "utf8"
    });
    targetSourceFile.addStatements(
      debug
        ? `\n// @url ${relative(basePath, filename)}\n\n${text}`
        : `\n${text}`
    );
  }
}

/** Load a set of files into a file system host. */
export function loadFiles(
  project: Project,
  filePaths: string[],
  rebase?: string
): void {
  const fileSystem = project.getFileSystem();
  for (const filePath of filePaths) {
    const fileText = readFileSync(filePath, {
      encoding: "utf8"
    });
    fileSystem.writeFileSync(
      rebase ? join(rebase, basename(filePath)) : filePath,
      fileText
    );
  }
}

/**
 * Load and write to a virtual file system all the default libs needed to
 * resolve types on project.
 */
export function loadDtsFiles(
  project: Project,
  compilerOptions: ts.CompilerOptions
): void {
  const libSourcePath = dirname(ts.getDefaultLibFilePath(compilerOptions));
  // TODO (@kitsonk) Add missing libs when ts-morph supports TypeScript 3.4
  loadFiles(
    project,
    [
      "lib.es2015.collection.d.ts",
      "lib.es2015.core.d.ts",
      "lib.es2015.d.ts",
      "lib.es2015.generator.d.ts",
      "lib.es2015.iterable.d.ts",
      "lib.es2015.promise.d.ts",
      "lib.es2015.proxy.d.ts",
      "lib.es2015.reflect.d.ts",
      "lib.es2015.symbol.d.ts",
      "lib.es2015.symbol.wellknown.d.ts",
      "lib.es2016.array.include.d.ts",
      "lib.es2016.d.ts",
      "lib.es2017.d.ts",
      "lib.es2017.intl.d.ts",
      "lib.es2017.object.d.ts",
      "lib.es2017.sharedmemory.d.ts",
      "lib.es2017.string.d.ts",
      "lib.es2017.typedarrays.d.ts",
      "lib.es2018.d.ts",
      "lib.es2018.intl.d.ts",
      "lib.es2018.promise.d.ts",
      "lib.es5.d.ts",
      "lib.esnext.d.ts",
      "lib.esnext.array.d.ts",
      "lib.esnext.asynciterable.d.ts",
      "lib.esnext.intl.d.ts",
      "lib.esnext.symbol.d.ts"
    ].map(fileName => join(libSourcePath, fileName)),
    "node_modules/typescript/lib/"
  );
}

export interface NamespaceSourceFileOptions {
  debug?: boolean;
  namespace?: string;
  namespaces: Set<string>;
  rootPath: string;
  sourceFileMap: Map<SourceFile, string>;
}

/**
 * Take a source file (`.d.ts`) and convert it to a namespace, resolving any
 * imports as their own namespaces.
 */
export function namespaceSourceFile(
  sourceFile: SourceFile,
  {
    debug,
    namespace,
    namespaces,
    rootPath,
    sourceFileMap
  }: NamespaceSourceFileOptions
): string {
  if (sourceFileMap.has(sourceFile)) {
    return "";
  }
  if (!namespace) {
    namespace = sourceFile.getBaseNameWithoutExtension();
  }
  sourceFileMap.set(sourceFile, namespace);

  sourceFile.forEachChild(node => {
    if (TypeGuards.isAmbientableNode(node)) {
      node.setHasDeclareKeyword(false);
    }
  });

  // TODO need to properly unwrap this
  const globalNamespace = sourceFile.getNamespace("global");
  let globalNamespaceText = "";
  if (globalNamespace) {
    const structure = globalNamespace.getStructure();
    if (structure.bodyText && typeof structure.bodyText === "string") {
      globalNamespaceText = structure.bodyText;
    } else {
      throw new TypeError("Unexpected global declaration structure.");
    }
  }
  if (globalNamespace) {
    globalNamespace.remove();
  }

  const output = sourceFile
    .getImportDeclarations()
    .filter(declaration => {
      const dsf = declaration.getModuleSpecifierSourceFile();
      if (dsf == null) {
        try {
          const namespaceName = declaration
            .getNamespaceImportOrThrow()
            .getText();
          if (!namespaces.has(namespaceName)) {
            throw createDeclarationError(
              "Already defined source file under different namespace.",
              declaration
            );
          }
        } catch (e) {
          throw createDeclarationError(
            "Unsupported import clause.",
            declaration
          );
        }
        declaration.remove();
      }
      return dsf;
    })
    .map(declaration => {
      if (
        declaration.getNamedImports().length ||
        !declaration.getNamespaceImport()
      ) {
        throw createDeclarationError("Unsupported import clause.", declaration);
      }
      const text = namespaceSourceFile(
        declaration.getModuleSpecifierSourceFileOrThrow(),
        {
          debug,
          namespace: declaration.getNamespaceImportOrThrow().getText(),
          namespaces,
          rootPath,
          sourceFileMap
        }
      );
      declaration.remove();
      return text;
    })
    .join("\n");
  sourceFile
    .getExportDeclarations()
    .forEach(declaration => declaration.remove());

  namespaces.add(namespace);

  return `${output}
    ${globalNamespaceText || ""}

    declare namespace ${namespace} {
      ${debug ? getSourceComment(sourceFile, rootPath) : ""}
      ${sourceFile.getText()}
    }`;
}

/** Mirrors TypeScript's handling of paths */
export function normalizeSlashes(path: string): string {
  return path.replace(/\\/g, "/");
}
