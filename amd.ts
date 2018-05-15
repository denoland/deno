import * as path from "path";
import { assert, log } from "./util";

namespace ModuleExportsCache {
  const cache = new Map<string, object>();
  export function set(fileName: string, moduleExports: object) {
    fileName = normalizeModuleName(fileName);
    assert(
      fileName.startsWith("/"),
      `Normalized modules should start with /\n${fileName}`
    );
    log("ModuleExportsCache set", fileName);
    cache.set(fileName, moduleExports);
  }
  export function get(fileName: string): object {
    fileName = normalizeModuleName(fileName);
    log("ModuleExportsCache get", fileName);
    let moduleExports = cache.get(fileName);
    if (moduleExports == null) {
      moduleExports = {};
      set(fileName, moduleExports);
    }
    return moduleExports;
  }
}

function normalizeModuleName(fileName: string): string {
  // Remove the extension.
  return fileName.replace(/\.\w+$/, "");
}

function normalizeRelativeModuleName(contextFn: string, depFn: string): string {
  if (depFn.startsWith("/")) {
    return depFn;
  } else {
    return path.resolve(path.dirname(contextFn), depFn);
  }
}

const executeQueue: Array<() => void> = [];

export function executeQueueDrain(): void {
  let fn;
  while ((fn = executeQueue.shift())) {
    fn();
  }
}

// tslint:disable-next-line:no-any
type AmdFactory = (...args: any[]) => undefined | object;
type AmdDefine = (deps: string[], factory: AmdFactory) => void;

export function makeDefine(fileName: string): AmdDefine {
  const localDefine = (deps: string[], factory: AmdFactory): void => {
    const localRequire = (x: string) => {
      log("localRequire", x);
    };
    const localExports = ModuleExportsCache.get(fileName);
    log("localDefine", fileName, deps, localExports);
    const args = deps.map(dep => {
      if (dep === "require") {
        return localRequire;
      } else if (dep === "exports") {
        return localExports;
      } else {
        dep = normalizeRelativeModuleName(fileName, dep);
        return ModuleExportsCache.get(dep);
      }
    });
    executeQueue.push(() => {
      log("execute", fileName);
      const r = factory(...args);
      if (r != null) {
        ModuleExportsCache.set(fileName, r);
        throw Error("x");
      }
    });
  };
  return localDefine;
}
