// Copyright 2018 Ryan Dahl <ry@tinyclouds.org>
// All rights reserved. MIT License.

import * as ts from "typescript";
import { VISITOR, visit } from "./core";
import { isNamedDeclaration, isNodeExported, setFilename } from "./util";

// tslint:disable:only-arrow-functions

const visited = new Map<ts.ModuleDeclaration, null>();

VISITOR("ModuleDeclaration", function(e, node: ts.ModuleDeclaration) {
  const symbol = this.checker.getSymbolAtLocation(node.name);
  const docs = symbol.getDocumentationComment(this.checker);
  const array = [];
  visit.call(this, array, node.name);
  const name = array[0];
  this.currentNamespace.push(name);
  array.length = 0;
  // Visit module child only once.
  if (!visited.has(node)) {
    this.privateNames.addSeparator();
    visit.call(this, array, node.body);
    this.privateNames.removeLastSeparator();
  }
  e.push({
    type: "module",
    documentation: ts.displayPartsToString(docs),
    name,
    statements: array
  });
  setFilename(this, name);
  this.currentNamespace.pop();
  visited.set(node, null);
});

VISITOR("ModuleBlock", function(e, block: ts.ModuleBlock | ts.SourceFile) {
  if (!block.statements) return;
  const array = [];
  // Only visit exported declarations in first round.
  for (let i = block.statements.length - 1;i >= 0;--i) {
    const node = block.statements[i];
    // Visit all nodes if the given file is a declaration file.
    if (this.sourceFile.isDeclarationFile ||
        isNodeExported(node) ||
        node.kind === ts.SyntaxKind.ImportDeclaration ||
        node.kind === ts.SyntaxKind.ExportDeclaration) {
      visit.call(this, array, node);
    } else if (isNamedDeclaration(node)) {
      // TODO Maybe documentation should contain private declarations that
      // are used in exported declarations.
      this.privateNames.lock();
      visit.call(this, [], node);
      this.privateNames.unlock();
    }
  }
  // Visit for second time this time top to bottom
  // also do not push anything to e, just look for definitions.
  this.privateNames.lock();
  for (let i = 0;i < block.statements.length;++i) {
    visit.call(this, [], block.statements[i]);
  }
  this.privateNames.unlock();
  array.reverse();
  e.push(...array);
  // TODO visit while this.privateNames is not empty
});

VISITOR("SourceFile", "ModuleBlock");

VISITOR("ExportDeclaration", function(e, node: ts.ExportDeclaration) {
  if (!node.exportClause) return;
  // Just visit export specifiers
  for (const s of node.exportClause.elements) {
    visit.call(this, e, s);
  }
});

VISITOR("ExportSpecifier", function(e, node: ts.ExportSpecifier) {
  const array = [];
  visit.call(this, array, node.name);
  const name = array[0];
  let propertyName = name;
  if (node.propertyName) {
    array.length = 0;
    visit.call(this, array, node.propertyName);
    propertyName = array[0];
  }
  const entity = {
    type: "export",
    name,
    propertyName
  };
  e.push(entity);
  // Search for propertyName
  this.privateNames.add(propertyName, entity);
});

VISITOR("ImportDeclaration", function(e, node: ts.ImportDeclaration) {
  if (!node.importClause) return;
  // Only string literal is accepted.
  if (!ts.isStringLiteral(node.moduleSpecifier)) return;
  // ImportDeclaration must not push anything to e.
  visit.call(this, [], node.importClause.namedBindings);
});

VISITOR("NamedImports", function(e, node: ts.NamedImports) {
  for (const s of node.elements) {
    visit.call(this, e, s);
  }
});

VISITOR("ImportSpecifier", function(e, node: ts.ImportSpecifier) {
  const moduleSpecifier = node.parent.parent.parent.moduleSpecifier;
  let fileName = (moduleSpecifier as ts.StringLiteral).text;
  if (node.propertyName) {
    // Maybe use an array (?)
    fileName += "#" + node.propertyName.text;
  }
  setFilename(this, node.name.text, fileName);
});

VISITOR("NamespaceImport", function(e, node: ts.NamespaceImport) {
  const moduleSpecifier = node.parent.parent.moduleSpecifier;
  const fileName = (moduleSpecifier as ts.StringLiteral).text;
  setFilename(this, node.name.text, fileName);
});
