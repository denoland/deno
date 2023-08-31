// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

import { $, ReleasesMdFile, Repo } from "./deps.ts";

export class DenoWorkspace {
  #repo: Repo;

  static get rootDirPath() {
    const currentDirPath = $.path.dirname($.path.fromFileUrl(import.meta.url));
    return $.path.resolve(currentDirPath, "../../");
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
      .filter((c) => c.name !== "test_util");
  }

  getCliCrate() {
    return this.getCrate("deno");
  }

  getCrate(name: string) {
    return this.#repo.getCrate(name);
  }

  getReleasesMdFile() {
    return new ReleasesMdFile(
      $.path.join(DenoWorkspace.rootDirPath, "Releases.md"),
    );
  }

  async runFormatter() {
    await this.#repo.command(
      "deno run --unstable --allow-write --allow-read --allow-run ./tools/format.js",
    );
  }
}
