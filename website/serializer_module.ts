// Copyright 2018 Ryan Dahl <ry@tinyclouds.org>
// All rights reserved. MIT License.

import * as ts from "typescript";
import { VISITOR, visit } from "./parser";
import { isNodeExported, setFilename } from "./util";

// tslint:disable:only-arrow-functions

const visited = new Map<ts.ModuleDeclaration, null>();

VISITOR("ModuleDeclaration", function(e, node: ts.ModuleDeclaration) {
  const symbol = this.checker.getSymbolAtLocation(node.name);
  const docs = symbol.getDocumentationComment(this.checker);
  const array = [];
  visit.call(this, array, node.name);
  const name = array[0];
  this.currentNamespace.push(name.text);
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
    name: name.text,
    statements: array
  });
  setFilename(this, name.refName);
  this.currentNamespace.pop();
  visited.set(node, null);
});

VISITOR("ModuleBlock", function(e, block: ts.ModuleBlock | ts.SourceFile) {
  if (!block.statements) return;
  const array = [];
  const privateNodes = [];
  // To track which nodes are included in array or privateNodes
  // to prevent duplication.
  const includedPrivateNodes = new Map<ts.Node, null>();
  // Only visit exported declarations in first round.
  for (let i = block.statements.length - 1;i >= 0;--i) {
    const node = block.statements[i];
    // Visit all nodes if the given file is a declaration file.
    if (this.sourceFile.isDeclarationFile ||
        isNodeExported(node) ||
        node.kind === ts.SyntaxKind.ImportDeclaration ||
        node.kind === ts.SyntaxKind.ExportDeclaration ||
        node.kind === ts.SyntaxKind.ExportAssignment) {
      visit.call(this, array, node);
      includedPrivateNodes.set(node, null);
    } else {
      this.privateNames.lock();
      this.privateNames.changed = false;
      const tmp = [];
      visit.call(this, tmp, node);
      if (this.privateNames.changed) {
        includedPrivateNodes.set(node, null);
        privateNodes.push(...tmp);
      }
      this.privateNames.unlock();
    }
  }
  // Visit for second time this time top to bottom
  // also do not push anything to e, just look for definitions.
  if (!this.privateNames.isEmpty()) {
    this.privateNames.lock();
    for (let i = 0;i < block.statements.length;++i) {
      const node = block.statements[i];
      const tmp = [];
      visit.call(this, tmp, node);
      if (this.privateNames.changed &&
          !includedPrivateNodes.has(node)) {
        privateNodes.push(...tmp);
      }
    }
    this.privateNames.unlock();
  }
  array.reverse();
  e.push(...array);
  privateNodes.forEach(doc => {
    if (typeof doc === "object") {
      doc.isPrivate = true;
    }
  });
  e.push(...privateNodes);
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
    name: name.text,
    propertyName: propertyName.text
  };
  e.push(entity);
  // Search for propertyName
  this.privateNames.add(propertyName.refName, entity);
});

VISITOR("ExportAssignment", function(e, node: ts.ExportAssignment) {
  const expressions = [];
  visit.call(this, expressions, node.expression);
  const expression = expressions[0];
  let docEntity;
  if (expression.type === "name") {
    docEntity = {
      type: "export",
      name: expression.text,
      propertyName: expression.refName,
      isDefault: true
    };
    this.privateNames.add(expression.refName, docEntity);
  } else {
    docEntity = {
      type: "export",
      expression: expression,
      isDefault: true
    };
  }
  e.push(docEntity);
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
