// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

import { path, Repo } from "./deps.ts";

export class DenoWorkspace {
  #repo: Repo;

  static get rootDirPath() {
    const currentDirPath = path.dirname(path.fromFileUrl(import.meta.url));
    return path.resolve(currentDirPath, "../../");
  }

  static async load(): Promise<DenoWorkspace> {
    return new DenoWorkspace(
      await Repo.load("deno", DenoWorkspace.rootDirPath),
    );
  }

  private constructor(repo: Repo) {
    this.#repo = repo;
  }

  get repo() {
    return this.#repo;
  }

  get crates() {
    return this.#repo.crates;
  }

  /** Gets the dependency crates used for the first part of the release process. */
  getDependencyCrates() {
    return [
      this.getBenchUtilCrate(),
      this.getSerdeV8Crate(),
      this.getCoreCrate(),
      ...this.getExtCrates(),
      this.getRuntimeCrate(),
    ];
  }

  getSerdeV8Crate() {
    return this.getCrate("serde_v8");
  }

  getCliCrate() {
    return this.getCrate("deno");
  }

  getCoreCrate() {
    return this.getCrate("deno_core");
  }

  getRuntimeCrate() {
    return this.getCrate("deno_runtime");
  }

  getBenchUtilCrate() {
    return this.getCrate("deno_bench_util");
  }

  getExtCrates() {
    const extPath = path.join(this.#repo.folderPath, "ext");
    return this.crates.filter((c) => c.manifestPath.startsWith(extPath));
  }

  getCrate(name: string) {
    return this.#repo.getCrate(name);
  }
}
