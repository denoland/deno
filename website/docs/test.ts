import * as ts from "typescript";
import { generateDoc } from "./core";
import "./serializers/function";
import "./serializers/types";
import "./serializers/keywords";

// tslint:disable-next-line:no-require-imports
const doc = generateDoc("r.ts", require("../tsconfig.json"));
console.log(doc);
console.log(JSON.stringify(doc, null, 2));

// To use chrome dev-tools
setInterval(() => null, 1e5);
global["ts"] = ts;
global["strKind"] = n => ts.SyntaxKind[n];
