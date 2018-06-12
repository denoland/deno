/// <reference path="deno.d.ts" />
//import { main as pb } from "deno_pb/msg.pb"
import * as ts from "typescript";

const globalEval = eval;
const window = globalEval("this");
window["denoMain"] = () => {
  //const msg = pb.Msg.fromObject({});
  //denoPrint(`msg.command: ${msg.command}`);
  denoPrint(`ts.version: ${ts.version}`);
  denoPrint("Hello world from foo");
  return "foo";
};
