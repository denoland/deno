// Copyright 2018 Ryan Dahl <ry@tinyclouds.org>
// All rights reserved. MIT License.

import * as ts from "typescript";
import { VISITOR, visit } from "./parser";
import { setFilename } from "./util";

// tslint:disable:only-arrow-functions

VISITOR("EnumDeclaration", function(e, node: ts.EnumDeclaration) {
  const symbol = this.checker.getSymbolAtLocation(node.name);
  const docs = symbol.getDocumentationComment(this.checker);

  const members = [];
  for (const t of node.members) {
    visit.call(this, members, t);
  }

  const modifierFlags = ts.getCombinedModifierFlags(node);
  const isDefault = (modifierFlags & ts.ModifierFlags.Default) !== 0;

  e.push({
    type: "enum",
    name: node.name.text,
    documentation: ts.displayPartsToString(docs),
    members,
    isDefault
  });
  setFilename(this, node.name.text);
});

VISITOR("EnumMember", function(e, node: ts.EnumMember) {
  const symbol = this.checker.getSymbolAtLocation(node.name);
  const docs = symbol.getDocumentationComment(this.checker);
  const array = [];
  visit.call(this, array, node.initializer);
  const initializer = array[0];
  array.length = 0;
  visit.call(this, array, node.name);
  const name = array[0]
  e.push({
    type: "EnumMember",
    documentation: ts.displayPartsToString(docs),
    initializer,
    name: name.text
  });
});
