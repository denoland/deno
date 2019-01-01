// Copyright 2018 the Deno authors. All rights reserved. MIT license.
// tslint:disable-next-line:no-circular-imports
import * as deno from "./deno";
import { globalEval } from "./global_eval";
import { assert, log } from "./util";

// tslint:disable:no-any
type AmdCallback = (...args: any[]) => void;
type AmdDefine = (deps: ModuleSpecifier[], factory: AmdFactory) => void;
type AmdErrback = (err: any) => void;
type AmdFactory = (...args: any[]) => object | void;
type AmdRequire = (
  deps: ModuleSpecifier[],
  callback: AmdCallback,
  errback: AmdErrback
) => void;
// tslint:enable:no-any

// tslint:disable-next-line:no-any
type BuiltinMap = { [moduleSpecifier: string]: any };

// Type aliases to make the code more readable
type ContainingFile = string;
type Filename = string;
type ModuleSpecifier = string;
type OutputCode = string;

/** Internal representation of a module being loaded */
class Module {
  deps?: Filename[];
  factory?: AmdFactory;

  // tslint:disable-next-line:no-any
  constructor(public filename: Filename, public exports: any) {}
}

/** External APIs which the runner depends upon to be able to retrieve
 * transpiled modules.
 */
export interface CodeProvider {
  /** Given a module specifier and a containing file, return the filename. */
  getFilename(
    moduleSpecifier: ModuleSpecifier,
    containingFile: ContainingFile
  ): Filename;

  /** Given a filename, return the transpiled output code. */
  getOutput(filename: Filename): OutputCode;
}

const window = globalEval("this");

/** A class which can load and run modules into the current environment. */
export class Runner {
  private _globalEval = globalEval;
  /** A map of modules indexed by filename. */
  private _modules = new Map<Filename, Module>();
  private _provider: CodeProvider;
  /** Modules are placed in here to have their factories run after all the
   * the dependencies have been collected.
   */
  private _runQueue: Module[] = [];

  private _drainRunQueue(): void {
    log("runner._drainRunQueue", this._runQueue.length);
    let module: Module | undefined;
    while ((module = this._runQueue.shift())) {
      assert(module.factory != null, "Cannot run module without factory.");
      // TypeScript always imports `exports` and mutates it directly, but the
      // AMD specification allows values to be returned from the factory and
      // is the case with JSON modules and potentially other future features.
      const exports = module.factory!(...this._getFactoryArguments(module));
      if (exports != null) {
        module.exports = exports;
      }
    }
  }

  private _gatherDependencies(filename: Filename): void {
    log("runner._gatherDependencies", filename);

    if (this._modules.has(filename)) {
      log("Module already exists:", filename);
      return;
    }

    const module = new Module(filename, {});
    this._modules.set(filename, module);

    window.define = this._makeDefine(module);
    this._globalEval(this._provider.getOutput(filename));
    window.define = undefined;
  }

  // tslint:disable-next-line:no-any
  private _getFactoryArguments(module: Module): any[] {
    log("runner._getFactoryArguments", module.filename);
    assert(module.deps != null, "Missing dependencies for module.");
    return module.deps!.map(dep => {
      if (dep === "require") {
        return this._makeLocalRequire(module);
      }
      if (dep === "exports") {
        return module.exports;
      }
      if (dep in Runner._builtins) {
        return Runner._builtins[dep];
      }
      const depModule = this._modules.get(dep)!;
      assert(dep != null, `Missing dependency "${dep}"`);
      return depModule.exports;
    });
  }

  private _makeDefine(module: Module): AmdDefine {
    log("runner._makeDefine", module.filename);
    return (deps: ModuleSpecifier[], factory: AmdFactory): void => {
      module.factory = factory;
      module.deps = deps.map(dep => {
        if (dep === "require" || dep === "exports" || dep in Runner._builtins) {
          return dep;
        }
        const depFilename = this._provider.getFilename(dep, module.filename);
        if (!this._modules.get(depFilename)) {
          this._gatherDependencies(depFilename);
        }
        return depFilename;
      });
      if (!this._runQueue.includes(module)) {
        this._runQueue.push(module);
      }
    };
  }

  private _makeLocalRequire(module: Module): AmdRequire {
    log("runner._makeLocalRequire", module.filename);
    return (
      deps: ModuleSpecifier[],
      callback: AmdCallback,
      errback: AmdErrback
    ): void => {
      log("runner._makeLocalRequire", deps);
      assert(
        deps.length === 1,
        "Local require supports exactly one dependency."
      );
      const [moduleSpecifier] = deps;
      try {
        this.run(moduleSpecifier, module.filename);
        const requiredFilename = this._provider.getFilename(
          moduleSpecifier,
          module.filename
        );
        const requiredModule = this._modules.get(requiredFilename)!;
        assert(requiredModule != null);
        callback(requiredModule.exports);
      } catch (e) {
        errback(e);
      }
    };
  }

  constructor(provider: CodeProvider) {
    this._provider = provider;
  }

  /** Given a module specifier and the containing file, resolve the module and
   * ensure that it is in the runtime environment, returning the exports of the
   * module.
   */
  // tslint:disable-next-line:no-any
  run(moduleSpecifier: ModuleSpecifier, containingFile: ContainingFile): any {
    log("runner.run", moduleSpecifier, containingFile);
    const filename = this._provider.getFilename(
      moduleSpecifier,
      containingFile
    );
    if (!this._modules.has(filename)) {
      this._gatherDependencies(filename);
      this._drainRunQueue();
    }
    return this._modules.get(filename)!.exports;
  }

  /** Builtin modules which can be loaded by user modules. */
  private static _builtins: BuiltinMap = { deno };
}
