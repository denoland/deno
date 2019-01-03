import { relative } from "path";
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
  TypeGuards,
  VariableStatement,
  VariableDeclarationKind
} from "ts-simple-ast";

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
) {
  return node.addTypeAlias({
    name,
    type,
    docs: jsdocs && jsdocs.map(jsdoc => jsdoc.getText()),
    hasDeclareKeyword
  });
}

/** Add a declaration of a variable to a node */
export function addVariableDeclaration(
  node: StatementedNode,
  name: string,
  type: string,
  hasDeclareKeyword?: boolean,
  jsdocs?: JSDoc[]
): VariableStatement {
  return node.addVariableStatement({
    declarationKind: VariableDeclarationKind.Const,
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

/** Check diagnostics, and if any exist, exit the process */
export function checkDiagnostics(project: Project, onlyFor?: string[]) {
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
  ) {
    const declarationSourceFile = declaration.getModuleSpecifierSourceFile();
    if (declarationSourceFile) {
      processSourceFile(declarationSourceFile);
      declaration.remove();
    }
  }

  function rectifyNodes(currentSourceFile: SourceFile) {
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

  function processSourceFile(currentSourceFile: SourceFile) {
    if (sourceFiles.has(currentSourceFile)) {
      return;
    }
    sourceFiles.add(currentSourceFile);

    const currentSourceFilePath = currentSourceFile.getFilePath();
    if (customSources && currentSourceFilePath in customSources) {
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

/** Returns a string which indicates the source file as the source */
export function getSourceComment(
  sourceFile: SourceFile,
  rootPath: string
): string {
  return `\n// @url ${relative(rootPath, sourceFile.getFilePath())}\n\n`;
}

/**
 * Load and write to a virtual file system all the default libs needed to
 * resolve types on project.
 */
export function loadDtsFiles(project: Project) {
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
      "lib.es2018.regexp.d.ts",
      "lib.es5.d.ts",
      "lib.esnext.d.ts",
      "lib.esnext.array.d.ts",
      "lib.esnext.asynciterable.d.ts",
      "lib.esnext.intl.d.ts",
      "lib.esnext.symbol.d.ts"
    ].map(fileName => `node_modules/typescript/lib/${fileName}`)
  );
}

/** Load a set of files into a file system host. */
export function loadFiles(project: Project, filePaths: string[]) {
  const fileSystem = project.getFileSystem();
  for (const filePath of filePaths) {
    const fileText = readFileSync(filePath, {
      encoding: "utf8"
    });
    fileSystem.writeFileSync(filePath, fileText);
  }
}

/** Log diagnostics to the console with colour. */
export function logDiagnostics(diagnostics: ts.Diagnostic[]): void {
  if (diagnostics.length) {
    console.log(
      ts.formatDiagnosticsWithColorAndContext(diagnostics, formatDiagnosticHost)
    );
  }
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
