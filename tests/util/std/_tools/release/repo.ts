// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import { path, ReleasesMdFile, Repo, semver } from "./deps.ts";

const currentDirPath = path.dirname(path.fromFileUrl(import.meta.url));
export const rootDirPath = path.resolve(currentDirPath, "../../");

export class VersionFile {
  #filePath: string;
  #fileText: string;

  static #versionRe = /"([0-9]+\.[0-9]+\.[0-9]+)"/;

  constructor() {
    this.#filePath = path.join(rootDirPath, "version.ts");
    this.#fileText = Deno.readTextFileSync(this.#filePath);
  }

  get version() {
    const version = VersionFile.#versionRe.exec(this.#fileText);
    if (version === null) {
      throw new Error(`Could not find version in text: ${this.#fileText}`);
    } else {
      return semver.parse(version[1])!;
    }
  }

  updateVersion(version: semver.SemVer) {
    this.#fileText = this.#fileText.replace(
      VersionFile.#versionRe,
      `"${version}"`,
    );
    Deno.writeTextFileSync(this.#filePath, this.#fileText);
  }
}

export function loadRepo() {
  return Repo.load({
    name: "deno_std",
    path: rootDirPath,
  });
}

export function getReleasesMdFile() {
  return new ReleasesMdFile(path.join(rootDirPath, "./Releases.md"));
}
