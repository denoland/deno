// Copyright 2018-2021 the Deno authors. All rights reserved. MIT license.

import * as path from "https://deno.land/std@0.105.0/path/mod.ts";
import * as semver from "https://deno.land/x/semver@v1.4.0/mod.ts";
import * as cargo from "./cargo.ts";
import { getCratesIoMetadata } from "./crates_io.ts";
import { withRetries } from "./helpers.ts";

export class DenoWorkspace {
  #workspaceCrates: readonly DenoWorkspaceCrate[];
  #workspaceRootDirPath: string;

  static get rootDirPath() {
    const currentDirPath = path.dirname(path.fromFileUrl(import.meta.url));
    return path.resolve(currentDirPath, "../../../");
  }

  static async load(): Promise<DenoWorkspace> {
    return new DenoWorkspace(
      await cargo.getMetadata(DenoWorkspace.rootDirPath),
    );
  }

  private constructor(metadata: cargo.CargoMetadata) {
    const crates = [];
    for (const memberId of metadata.workspace_members) {
      const pkg = metadata.packages.find((pkg) => pkg.id === memberId);
      if (!pkg) {
        throw new Error(`Could not find package with id ${memberId}`);
      }
      crates.push(new DenoWorkspaceCrate(this, pkg));
    }

    this.#workspaceCrates = crates;
    this.#workspaceRootDirPath = metadata.workspace_root;
  }

  get crates() {
    return this.#workspaceCrates;
  }

  /** Gets the dependency crates used for the first part of the release process. */
  getDependencyCrates() {
    return [
      this.getBenchUtilCrate(),
      this.getCoreCrate(),
      ...this.getExtCrates(),
      this.getRuntimeCrate(),
    ];
  }

  getCliCrate() {
    return this.getCrateByNameOrThrow("deno");
  }

  getCoreCrate() {
    return this.getCrateByNameOrThrow("deno_core");
  }

  getRuntimeCrate() {
    return this.getCrateByNameOrThrow("deno_runtime");
  }

  getBenchUtilCrate() {
    return this.getCrateByNameOrThrow("deno_bench_util");
  }

  getExtCrates() {
    const extPath = path.join(this.#workspaceRootDirPath, "ext");
    return this.#workspaceCrates.filter((c) =>
      c.manifestPath.startsWith(extPath)
    );
  }

  getCrateByNameOrThrow(name: string) {
    const crate = this.#workspaceCrates.find((c) => c.name === name);
    if (!crate) {
      throw new Error(`Could not find crate: ${name}`);
    }
    return crate;
  }

  build() {
    return cargo.build(DenoWorkspace.rootDirPath);
  }

  updateLockFile() {
    return cargo.check(DenoWorkspace.rootDirPath);
  }
}

export class DenoWorkspaceCrate {
  #workspace: DenoWorkspace;
  #pkg: cargo.CargoPackageMetadata;
  #isUpdatingManifest = false;

  constructor(workspace: DenoWorkspace, pkg: cargo.CargoPackageMetadata) {
    this.#workspace = workspace;
    this.#pkg = pkg;
  }

  get manifestPath() {
    return this.#pkg.manifest_path;
  }

  get directoryPath() {
    return path.dirname(this.#pkg.manifest_path);
  }

  get name() {
    return this.#pkg.name;
  }

  get version() {
    return this.#pkg.version;
  }

  getDependencies() {
    const dependencies = [];
    for (const dependency of this.#pkg.dependencies) {
      const crate = this.#workspace.crates.find((c) =>
        c.name === dependency.name
      );
      if (crate != null) {
        dependencies.push(crate);
      }
    }
    return dependencies;
  }

  async isPublished() {
    const cratesIoMetadata = await getCratesIoMetadata(this.name);
    return cratesIoMetadata.versions.some((v) => v.num === this.version);
  }

  async publish() {
    if (await this.isPublished()) {
      console.log(`Already published ${this.name} ${this.version}`);
      return false;
    }

    console.log(`Publishing ${this.name} ${this.version}...`);

    // Sometimes a publish may fail due to local caching issues.
    // Usually it will fix itself after retrying so try a few
    // times before failing hard.
    return await withRetries({
      action: async () => {
        await cargo.publishCrate(this.directoryPath);
        return true;
      },
      retryCount: 3,
      retryDelaySeconds: 10,
    });
  }

  build() {
    return cargo.build(this.directoryPath);
  }

  updateLockFile() {
    return cargo.check(this.directoryPath);
  }

  increment(part: "major" | "minor" | "patch") {
    const newVersion = semver.parse(this.version)!.inc(part).toString();
    return this.setVersion(newVersion);
  }

  async setVersion(version: string) {
    console.log(`Setting ${this.name} to ${version}...`);
    for (const crate of this.#workspace.crates) {
      await crate.setDependencyVersion(this.name, version);
    }
    await this.#updateManifestVersion(version);
  }

  async setDependencyVersion(dependencyName: string, version: string) {
    const dependency = this.#pkg.dependencies.find((d) =>
      d.name === dependencyName
    );
    if (dependency != null) {
      await this.#updateManifestFile((fileText) => {
        // simple for now...
        const findRegex = new RegExp(
          `^(\\b${dependencyName}\\b\\s.*)"([=\\^])?[0-9]+[^"]+"`,
          "gm",
        );
        return fileText.replace(findRegex, `$1"${version}"`);
      });

      dependency.req = `^${version}`;
    }
  }

  async #updateManifestVersion(version: string) {
    await this.#updateManifestFile((fileText) => {
      const findRegex = new RegExp(
        `^(version\\s*=\\s*)"${this.#pkg.version}"$`,
        "m",
      );
      return fileText.replace(findRegex, `$1"${version}"`);
    });
    this.#pkg.version = version;
  }

  async #updateManifestFile(action: (fileText: string) => string) {
    if (this.#isUpdatingManifest) {
      throw new Error("Cannot update manifest while updating manifest.");
    }
    this.#isUpdatingManifest = true;
    try {
      const originalText = await Deno.readTextFile(this.#pkg.manifest_path);
      const newText = action(originalText);
      if (originalText === newText) {
        throw new Error(`The file didn't change: ${this.manifestPath}`);
      }
      await Deno.writeTextFile(this.manifestPath, newText);
    } finally {
      this.#isUpdatingManifest = false;
    }
  }
}
