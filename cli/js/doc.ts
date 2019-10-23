// Copyright 2018-2019 the Deno authors. All rights reserved. MIT license.

import "./ts_global.d.ts";
import "./globals.ts";

import * as util from "./util.ts";

const { assert } = util;

enum DocNodeType {
  Class = "class",
  Parameter = "parameter",
  Signature = "signature",
  Module = "module",
  Export = "export",
  NamedExport = "namedexport"
}

export interface DocNode<T = DocNodeType> {
  documentation?: string;
  name?: string;
  nodeType: T;
  type?: string;
  pos: number;
  end: number;
}

// eslint-disable-next-line @typescript-eslint/no-empty-interface
export interface ParameterDocNode extends DocNode<DocNodeType.Parameter> {}

export interface SignatureDocNode extends DocNode<DocNodeType.Signature> {
  parameters: ParameterDocNode[];
  returnType: string;
}

export interface ClassDocNode extends DocNode<DocNodeType.Class> {
  constructors: SignatureDocNode[];
}

export interface ModuleDocNode extends DocNode<DocNodeType.Module> {
  exports: DocNode[];
}

export interface NamedExportDocNode extends DocNode<DocNodeType.NamedExport> {
  name: string;
  propertyName?: string;
}

export interface ExportDocNode extends DocNode<DocNodeType.Export> {
  moduleSpecifier: string;
  namedExports?: NamedExportDocNode[];
}

/** Determines if a node is exported from a module */
function isExportedNode(node: ts.Node | ts.Declaration): boolean {
  return (
    ("_declarationBrand" in node &&
      ts.getCombinedModifierFlags(node) & ts.ModifierFlags.Export) !== 0 ||
    (!!node.parent && node.parent.kind === ts.SyntaxKind.SourceFile)
  );
}

/** General purpose function which serializes a symbol into a doc node. */
function serializeSymbol<T>(
  symbol: ts.Symbol,
  checker: ts.TypeChecker,
  nodeType: T
): DocNode<T> {
  const documentation =
    ts.displayPartsToString(symbol.getDocumentationComment(checker)) ||
    undefined;
  return {
    name: symbol.getName(),
    documentation,
    type: checker.typeToString(
      checker.getTypeOfSymbolAtLocation(symbol, symbol.valueDeclaration!)
    ),
    nodeType,
    pos: symbol.valueDeclaration.pos,
    end: symbol.valueDeclaration.end
  };
}

/** Serialize a parameter of a function or method. */
function serializeParameters(
  symbol: ts.Symbol,
  checker: ts.TypeChecker
): ParameterDocNode {
  return serializeSymbol(symbol, checker, DocNodeType.Parameter);
}

/** Given a signature of a function or method, serialize the signature. */
function serializeSignature(
  signature: ts.Signature,
  checker: ts.TypeChecker
): SignatureDocNode {
  assert(signature.declaration != null);
  return {
    parameters: signature.parameters.map(symbol =>
      serializeParameters(symbol, checker)
    ),
    returnType: checker.typeToString(signature.getReturnType()),
    documentation: ts.displayPartsToString(
      signature.getDocumentationComment(checker)
    ),
    nodeType: DocNodeType.Signature,
    pos: signature.declaration!.pos,
    end: signature.declaration!.end
  };
}

/** For a given class, serialize the class. */
function serializeClass(
  node: ts.ClassDeclaration,
  checker: ts.TypeChecker
): ClassDocNode {
  assert(node.name != null);
  const symbol = checker.getSymbolAtLocation(node.name!)!;
  assert(symbol != null);
  const docNode = serializeSymbol(
    symbol,
    checker,
    DocNodeType.Class
  ) as ClassDocNode;

  node.members;

  const constructorType = checker.getTypeOfSymbolAtLocation(
    symbol,
    symbol.valueDeclaration
  );
  docNode.constructors = constructorType
    .getConstructSignatures()
    .map(signature => serializeSignature(signature, checker));
  return docNode;
}

/** Serialize an export declaration. */
function serializeExport(node: ts.ExportDeclaration): ExportDocNode {
  assert(node.moduleSpecifier != null);
  const moduleSpecifier = node.moduleSpecifier!.getText();
  return {
    nodeType: DocNodeType.Export,
    pos: node.pos,
    end: node.end,
    moduleSpecifier: moduleSpecifier.substring(1, moduleSpecifier.length - 1),
    namedExports: node.exportClause
      ? node.exportClause.elements.map(namedExport => ({
          nodeType: DocNodeType.NamedExport,
          name: namedExport.name.getText(),
          propertyName: namedExport.propertyName
            ? namedExport.propertyName.getText()
            : undefined,
          pos: namedExport.pos,
          end: namedExport.end
        }))
      : undefined
  };
}

/** Visit a node and determine if it is exported.  If it is, serialize it and
 * add it to the exported modules for the module doc node */
function visit(
  node: ts.Node,
  checker: ts.TypeChecker,
  moduleNode: ModuleDocNode
): void {
  if (!isExportedNode(node)) {
    return;
  }

  const { exports } = moduleNode;

  if (
    ts.isClassDeclaration(node) &&
    node.name &&
    checker.getSymbolAtLocation(node.name)
  ) {
    exports.push(serializeClass(node, checker));
  } else if (ts.isExportDeclaration(node)) {
    exports.push(serializeExport(node));
  } else {
    util.log(`unhandled doc node: ${node.kind} ${ts.SyntaxKind[node.kind]}`);
  }
}

/** For a given program, return an array of module documentation elements.
 * @param program The TypeScript compiler program
 * @param rootModule The main module of the documentation.  Any modules that are
 *   peers or relative submodules of this module will be documented.
 */
export function generateDoc(
  program: ts.Program,
  rootModule: string
): ModuleDocNode[] {
  const output: ModuleDocNode[] = [];
  const checker = program.getTypeChecker();

  const parts = rootModule.split("/");
  parts.pop();
  const rootPath = parts.join("/");

  for (const sourceFile of program.getSourceFiles()) {
    if (!sourceFile.isDeclarationFile) {
      const name = relative(rootPath, sourceFile.fileName);
      const moduleNode: ModuleDocNode = {
        name,
        nodeType: DocNodeType.Module,
        exports: [],
        pos: sourceFile.pos,
        end: sourceFile.end
      };
      output.push(moduleNode);
      ts.forEachChild(sourceFile, node => visit(node, checker, moduleNode));
    }
  }

  return output;
}

const CHAR_FORWARD_SLASH = 47;

/** A standalone version of `relative()` from `std/fs/path` that knows that
 * inbound paths don't need to be normalized. */
function relative(from: string, to: string): string {
  if (from === to) return "";

  function ltrim(input: string, charCode: number): [number, number, number] {
    let start = 1;
    const end = input.length;
    for (; start < end; ++start) {
      if (input.charCodeAt(start) !== charCode) break;
    }
    const len = end - start;
    return [start, end, len];
  }

  // Trim any leading backslashes
  const [fromStart, fromEnd, fromLen] = ltrim(from, CHAR_FORWARD_SLASH);

  // Trim any leading backslashes
  const [toStart, , toLen] = ltrim(to, CHAR_FORWARD_SLASH);

  // Compare paths to find the longest common path from root
  const length = fromLen < toLen ? fromLen : toLen;
  let lastCommonSep = -1;
  let i = 0;
  for (; i <= length; ++i) {
    if (i === length) {
      if (toLen > length) {
        if (to.charCodeAt(toStart + i) === CHAR_FORWARD_SLASH) {
          // We get here if `from` is the exact base path for `to`.
          // For example: from='/foo/bar'; to='/foo/bar/baz'
          return to.slice(toStart + i + 1);
        } else if (i === 0) {
          // We get here if `from` is the root
          // For example: from='/'; to='/foo'
          return to.slice(toStart + i);
        }
      } else if (fromLen > length) {
        if (from.charCodeAt(fromStart + i) === CHAR_FORWARD_SLASH) {
          // We get here if `to` is the exact base path for `from`.
          // For example: from='/foo/bar/baz'; to='/foo/bar'
          lastCommonSep = i;
        } else if (i === 0) {
          // We get here if `to` is the root.
          // For example: from='/foo'; to='/'
          lastCommonSep = 0;
        }
      }
      break;
    }
    const fromCode = from.charCodeAt(fromStart + i);
    const toCode = to.charCodeAt(toStart + i);
    if (fromCode !== toCode) break;
    else if (fromCode === CHAR_FORWARD_SLASH) lastCommonSep = i;
  }

  let out = "";
  // Generate the relative path based on the path difference between `to`
  // and `from`
  for (i = fromStart + lastCommonSep + 1; i <= fromEnd; ++i) {
    if (i === fromEnd || from.charCodeAt(i) === CHAR_FORWARD_SLASH) {
      if (out.length === 0) out += "..";
      else out += "/..";
    }
  }

  // Lastly, append the rest of the destination (`to`) path that comes after
  // the common path parts
  if (out.length > 0) return out + to.slice(toStart + lastCommonSep);
  else {
    let start = toStart + lastCommonSep;
    if (to.charCodeAt(start) === CHAR_FORWARD_SLASH) ++start;
    return to.slice(start);
  }
}
