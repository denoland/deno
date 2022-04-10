// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

import { path, semver } from "./deps.ts";
import type { Repo } from "./repo.ts";
import {
  existsSync,
  runCommand,
  runCommandWithOutput,
  withRetries,
} from "./helpers.ts";
import { CargoPackageMetadata } from "./cargo.ts";
import { getCratesIoMetadata } from "./crates_io.ts";

export class Crate {
  #pkg: CargoPackageMetadata;
  #isUpdatingManifest = false;

  constructor(
    public readonly repo: Repo,
    crateMetadata: CargoPackageMetadata,
  ) {
    if (!existsSync(crateMetadata.manifest_path)) {
      throw new Error(`Could not find crate at ${crateMetadata.manifest_path}`);
    }
    this.#pkg = crateMetadata;
  }

  get manifestPath() {
    return this.#pkg.manifest_path;
  }

  get folderPath() {
    return path.dirname(this.#pkg.manifest_path);
  }

  get name() {
    return this.#pkg.name;
  }

  get version() {
    return this.#pkg.version;
  }

  /** Prompts the user how they would like to patch and increments the version accordingly. */
  async promptAndIncrement() {
    const result = await this.promptAndTryIncrement();
    if (result == null) {
      throw new Error("No decision.");
    }
    return result;
  }

  /** Prompts the user how they would like to patch and increments the version accordingly. */
  async promptAndTryIncrement() {
    console.log(`${this.name} is on ${this.version}`);
    const versionIncrement = getVersionIncrement();
    if (versionIncrement != null) {
      await this.increment(versionIncrement);
      console.log(`Set version to ${this.version}`);
    }
    return versionIncrement;

    function getVersionIncrement() {
      if (confirm("Increment patch?")) {
        return "patch";
      } else if (confirm("Increment minor?")) {
        return "minor";
      } else if (confirm("Increment major?")) {
        return "major";
      } else {
        return undefined;
      }
    }
  }

  increment(part: "major" | "minor" | "patch") {
    const newVersion = semver.parse(this.version)!.inc(part).toString();
    return this.setVersion(newVersion);
  }

  async setVersion(version: string) {
    console.log(`Setting ${this.name} to ${version}...`);
    for (const crate of this.repo.crates) {
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

  toLocalSource(crate: Crate) {
    return this.#updateManifestFile((fileText) => {
      const relativePath = path.relative(this.folderPath, crate.folderPath)
        .replace(/\\/g, "/");
      // try to replace if it had a property in the object
      const versionPropRegex = new RegExp(
        `^(${crate.name}\\b\\s.*)version\\s*=\\s*"[^"]+"`,
        "m",
      );
      const newFileText = fileText.replace(
        versionPropRegex,
        `$1path = "${relativePath}"`,
      );
      if (newFileText !== fileText) {
        return newFileText;
      }

      // now try to find if it just had a version
      const versionStringRegex = new RegExp(
        `^(\\b${crate.name}\\b\\s.*)"([=\\^])?[0-9]+[^"]+"`,
        "m",
      );
      return fileText.replace(
        versionStringRegex,
        `$1{ path = "${relativePath}" }`,
      );
    });
  }

  revertLocalSource(crate: Crate) {
    return this.#updateManifestFile((fileText) => {
      const crateVersion = crate.version.toString();
      // try to replace if it had a property in the object
      const pathOnlyRegex = new RegExp(
        `^${crate.name} = { path = "[^"]+" }$`,
        "m",
      );
      const newFileText = fileText.replace(
        pathOnlyRegex,
        `${crate.name} = "${crateVersion}"`,
      );
      if (newFileText !== fileText) {
        return newFileText;
      }

      // now try to find if it had a path in an object
      const versionStringRegex = new RegExp(
        `^(${crate.name}\\b\\s.*)path\\s*=\\s*"[^"]+"`,
        "m",
      );
      return fileText.replace(
        versionStringRegex,
        `$1version = "${crateVersion}"`,
      );
    });
  }

  /** Gets all the descendant dependencies in the repository. */
  descendantDependenciesInRepo() {
    // try to maintain publish order.
    const crates = new Map<string, Crate>();
    const stack = [...this.immediateDependenciesInRepo()];
    while (stack.length > 0) {
      const item = stack.pop()!;
      if (!crates.has(item.name)) {
        crates.set(item.name, item);
        stack.push(...item.immediateDependenciesInRepo());
      }
    }
    return Array.from(crates.values());
  }

  /** Gets the immediate child dependencies found in the repo. */
  immediateDependenciesInRepo() {
    const dependencies = [];
    for (const dependency of this.#pkg.dependencies) {
      const crate = this.repo.crates.find((c) => c.name === dependency.name);
      if (crate != null) {
        dependencies.push(crate);
      }
    }
    return dependencies;
  }

  async isPublished() {
    const cratesIoMetadata = await getCratesIoMetadata(this.name);
    return cratesIoMetadata.versions.some((v) =>
      v.num === this.version.toString()
    );
  }

  async publish(...additionalArgs: string[]) {
    if (await this.isPublished()) {
      console.log(`Already published ${this.name} ${this.version}`);
      return false;
    }

    console.log(`Publishing ${this.name} ${this.version}...`);

    // Sometimes a publish may fail due to the crates.io index
    // not being updated yet. Usually it will be resolved after
    // retrying, so try a few times before failing hard.
    return await withRetries({
      action: async () => {
        await this.runCommandWithOutput([
          "cargo",
          "publish",
          ...additionalArgs,
        ]);
        return true;
      },
      retryCount: 5,
      retryDelaySeconds: 10,
    });
  }

  async publishDryRun() {
    if (await this.isPublished()) {
      console.log(`Already published ${this.name} ${this.version}`);
      return false;
    }

    console.log(`Dry publishing ${this.name} ${this.version}...`);
    await this.runCommandWithOutput(["cargo", "publish", "--dry-run"]);
  }

  cargoCheck(...additionalArgs: string[]) {
    return this.runCommandWithOutput(["cargo", "check", ...additionalArgs]);
  }

  cargoUpdate(...additionalArgs: string[]) {
    return this.runCommandWithOutput(["cargo", "update", ...additionalArgs]);
  }

  build(args?: { allFeatures?: boolean; additionalArgs?: string[] }) {
    const cliArgs = ["cargo", "build"];
    if (args?.allFeatures) {
      cliArgs.push("--all-features");
    }
    if (args?.additionalArgs) {
      cliArgs.push(...args.additionalArgs);
    }
    return this.runCommandWithOutput(cliArgs);
  }

  test(args?: { allFeatures?: boolean; additionalArgs?: string[] }) {
    const cliArgs = ["cargo", "test"];
    if (args?.allFeatures) {
      cliArgs.push("--all-features");
    }
    if (args?.additionalArgs) {
      cliArgs.push(...args.additionalArgs);
    }
    return this.runCommandWithOutput(cliArgs);
  }

  runCommand(cmd: string[]) {
    return runCommand({
      cwd: this.folderPath,
      cmd,
    });
  }

  runCommandWithOutput(cmd: string[]) {
    return runCommandWithOutput({
      cwd: this.folderPath,
      cmd,
    });
  }

  async #updateManifestFile(action: (fileText: string) => string) {
    if (this.#isUpdatingManifest) {
      throw new Error("Cannot update manifest while updating manifest.");
    }
    this.#isUpdatingManifest = true;
    try {
      const originalText = await Deno.readTextFile(this.manifestPath);
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
