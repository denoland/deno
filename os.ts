import { ModuleInfo } from "./types";
import { sendMsg } from "./dispatch";
import { main as pb } from "./msg.pb";
import { assert } from "./util";

export function exit(code = 0): void {
  sendMsg("os", { exit: { code } });
}

export function sourceCodeFetch(
  moduleSpecifier: string,
  containingFile: string
): ModuleInfo {
  const res = sendMsg("os", {
    sourceCodeFetch: { moduleSpecifier, containingFile }
  });
  assert(res.command === pb.Msg.Command.SOURCE_CODE_FETCH_RES);
  return {
    moduleName: res.sourceCodeFetchResModuleName,
    filename: res.sourceCodeFetchResFilename,
    sourceCode: res.sourceCodeFetchResSourceCode,
    outputCode: res.sourceCodeFetchResOutputCode
  };
}

export function sourceCodeCache(
  filename: string,
  sourceCode: string,
  outputCode: string
): void {
  sendMsg("os", {
    sourceCodeCache: { filename, sourceCode, outputCode }
  });
}
