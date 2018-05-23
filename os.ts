import { ModuleInfo } from "./types";
import { sendMsg } from "./dispatch";

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
  return res.sourceCodeFetchRes;
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
