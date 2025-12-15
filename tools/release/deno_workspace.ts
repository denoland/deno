// Copyright 2018-2025 the Deno authors. MIT license.

import { $, ReleasesMdFile, Repo } from "./deps.ts";

export class DenoWorkspace {
  #repo: Repo;

  static get rootDirPath() {
    const currentDirPath = $.path(import.meta.dirname!);
    return currentDirPath.parentOrThrow().parentOrThrow();
  }

  static async load(): Promise<DenoWorkspace> {
    return new DenoWorkspace(
      await Repo.load({
        name: "deno",
        path: DenoWorkspace.rootDirPath,
      }),
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

  /** Gets the CLI dependency crates that should be published. */
  getCliDependencyCrates() {
    return this.getCliCrate()
      .descendantDependenciesInRepo()
      .filter((c) => c.name !== "test_server" && c.name !== "test_macro");
  }

  getCliCrate() {
    return this.getCrate("deno");
  }

  getDenoRtCrate() {
    return this.getCrate("denort");
  }

  getDenoLibCrate() {
    return this.getCrate("deno_lib");
  }

  getCrate(name: string) {
    return this.#repo.getCrate(name);
  }

  getReleasesMdFile() {
    return new ReleasesMdFile(
      DenoWorkspace.rootDirPath.join("Releases.md").toString(),
    );
  }

  async runFormatter() {
    await this.#repo.command(
      "deno run --allow-write --allow-read --allow-net --allow-run ./tools/format.js",
    );
  }
}
