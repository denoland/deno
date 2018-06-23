// Copyright 2018 Ryan Dahl <ry@tinyclouds.org>
// All rights reserved. MIT License.

import { assertEqual, test } from "liltest";
import { generateDoc } from "./core";
import "./serializer_function";
import "./serializer_types";
import "./serializer_keywords";
import "./serializer_interface";
import "./serializer_enum";
import "./serializer_class";

// tslint:disable-next-line:no-require-imports
const options = require("./tsconfig.json");

test(async function test_enum() {
  const docs = generateDoc("testdata/enum.ts", options);
  // Test enum
  assertEqual(docs[0].type, "enum");
  assertEqual(docs[0].name, "Operator");
  assertEqual(docs[0].documentation, "Some values representing basic mathematical operations.");
  // Test enum members
  assertEqual(docs[0].members.length, 4);
  assertEqual(docs[0].members[0].type, "EnumMember");
  assertEqual(docs[0].members[0].name, "ADD");
  assertEqual(docs[0].members[0].documentation, "Comment for ADD");
  assertEqual(docs[0].members[1].type, "EnumMember");
  assertEqual(docs[0].members[1].name, "DIV");
  assertEqual(docs[0].members[1].documentation, "Comment for DIV");
  assertEqual(docs[0].members[2].type, "EnumMember");
  assertEqual(docs[0].members[2].name, "MUL");
  assertEqual(docs[0].members[2].documentation, "Comment for MUL");
  assertEqual(docs[0].members[3].type, "EnumMember");
  assertEqual(docs[0].members[3].name, "SUB");
  assertEqual(docs[0].members[3].documentation, "");
  // Test initializer
  assertEqual(docs[0].members[3].initializer.type, "number");
  assertEqual(docs[0].members[3].initializer.text, "3");
});
