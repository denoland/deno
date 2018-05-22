import { ModuleInfo } from "./types";
import { sendMsgFromObject } from "./dispatch";

export function exit(code = 0): void {
  sendMsgFromObject("os", { exit: { code } });
}

export function sourceCodeFetch(
  moduleSpecifier: string,
  containingFile: string
): ModuleInfo {
  const res = sendMsgFromObject("os", {
    sourceCodeFetch: { moduleSpecifier, containingFile }
  });
  return res.sourceCodeFetchRes;
}

export function sourceCodeCache(
  filename: string,
  sourceCode: string,
  outputCode: string
): void {
  sendMsgFromObject("os", {
    sourceCodeCache: { filename, sourceCode, outputCode }
  });
}
