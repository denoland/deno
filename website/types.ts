// Copyright 2018 Ryan Dahl <ry@tinyclouds.org>
// All rights reserved. MIT License.

import * as ts from "typescript";
import { One2ManyMap } from "./util";

export interface TSKit {
  sourceFile: ts.SourceFile;
  checker: ts.TypeChecker;
  privateNames: One2ManyMap<string, any>;
  typeParameters: string[];
  currentNamespace: string[];
  isJS: boolean;
  isDeclarationFile: boolean;
}

export type Visitor = (this: TSKit, docEntries: any[], node: ts.Node) => void;
