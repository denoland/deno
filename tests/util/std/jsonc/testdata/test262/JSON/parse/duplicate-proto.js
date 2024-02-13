// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Copyright (C) 2020 devsnek. All rights reserved.
// This code is governed by the BSD license found in the LICENSE file.

/*---
esid: sec-object-initializer-static-semantics-early-errors
description: >
  It is a Syntax Error if PropertyNameList of PropertyDefinitionList contains
  any duplicate entries for "__proto__" and at least two of those entries were
  obtained from productions of the form
    PropertyDefinition : PropertyName `:` AssignmentExpression .
  This rule is not applied if this PropertyDefinition is contained within a
  Script which is being evaluated for JSON.parse (see step 4 of JSON.parse).
---*/

var result = JSON.parse('{ "__proto__": 1, "__proto__": 2 }');

assert.sameValue(result.__proto__, 2);
