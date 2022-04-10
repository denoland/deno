/// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

import { CargoPackageMetadata, getCargoMetadata } from "./cargo.ts";
import { Crate } from "./crate.ts";
import { path } from "./deps.ts";
import {
  existsSync,
  GitLogOutput,
  GitTags,
  runCommand,
  runCommandWithOutput,
} from "./helpers.ts";

export interface RepoLoadOptions {
  /** Name of the repo. */
  name: string;
  /** Path to the directory of the repo on the local file system. */
  path: string;
  /** Whether crates should not be loaded if a Cargo.toml exists
   * in the root of the repo. If no Cargo.toml exists, then it won't
   * load the crates anyway. */
  skipLoadingCrates?: boolean;
}

export class Repo {
  #crates: Crate[] = [];

  private constructor(
    public readonly name: string,
    public readonly folderPath: string,
  ) {
  }

  static async load(options: RepoLoadOptions) {
    const folderPath = path.resolve(options.path);
    const repo = new Repo(options.name, folderPath);

    if (
      !options.skipLoadingCrates &&
      existsSync(path.join(folderPath, "Cargo.toml"))
    ) {
      await repo.loadCrates();
    }

    return repo;
  }

  async loadCrates() {
    const metadata = await getCargoMetadata(this.folderPath);
    for (const memberId of metadata.workspace_members) {
      const pkg = metadata.packages.find((pkg) => pkg.id === memberId);
      if (!pkg) {
        throw new Error(`Could not find package with id ${memberId}`);
      }
      this.addCrate(pkg);
    }
  }

  addCrate(crateMetadata: CargoPackageMetadata) {
    if (this.#crates.some((c) => c.name === crateMetadata.name)) {
      throw new Error(`Cannot add ${crateMetadata.name} twice to a repo.`);
    }
    this.#crates.push(
      new Crate(this, crateMetadata),
    );
  }

  async loadCrateInSubDir(name: string, subDir: string) {
    subDir = path.join(this.folderPath, subDir);
    const metadata = await getCargoMetadata(subDir);
    const pkg = metadata.packages.find((pkg) => pkg.name === name);
    if (!pkg) {
      throw new Error(`Could not find package with name ${name}`);
    }
    this.addCrate(pkg);
  }

  get crates(): ReadonlyArray<Crate> {
    return [...this.#crates];
  }

  getCrate(name: string) {
    const crate = this.#crates.find((c) => c.name === name);
    if (crate == null) {
      throw new Error(
        `Could not find crate with name: ${name}\n${this.crateNamesText()}`,
      );
    }
    return crate;
  }

  /** Gets the names of all the crates for showing in error messages
   * or for debugging purpopses. */
  crateNamesText() {
    return this.#crates.length === 0
      ? "<NO CRATES>"
      : this.#crates.map((c) => `- ${c.name}`).join("\n");
  }

  getCratesPublishOrder() {
    return getCratesPublishOrder(this.crates);
  }

  async hasLocalChanges() {
    const output = await this.runCommand([
      "git",
      "status",
      "--porcelain",
      "--untracked-files=no",
    ]);
    return output.trim().length > 0;
  }

  async assertCurrentBranch(expectedName: string) {
    const actualName = await this.gitCurrentBranch();
    if (actualName !== expectedName) {
      throw new Error(
        `Expected branch ${expectedName}, but current branch was ${actualName}.`,
      );
    }
  }

  async gitCurrentBranch() {
    return (await this.runCommand(["git", "rev-parse", "--abbrev-ref", "HEAD"]))
      .trim();
  }

  gitSwitch(...args: string[]) {
    return this.runCommand(["git", "switch", ...args]);
  }

  gitPull(...args: string[]) {
    return this.runCommand(["git", "pull", ...args]);
  }

  gitResetHard() {
    return this.runCommand(["git", "reset", "--hard"]);
  }

  gitBranch(name: string) {
    return this.runCommandWithOutput(["git", "checkout", "-b", name]);
  }

  gitAdd() {
    return this.runCommandWithOutput(["git", "add", "."]);
  }

  gitTag(name: string) {
    return this.runCommandWithOutput(["git", "tag", name]);
  }

  gitCommit(message: string) {
    return this.runCommandWithOutput(["git", "commit", "-m", message]);
  }

  gitPush(...additionalArgs: string[]) {
    return this.runCommandWithOutput(["git", "push", ...additionalArgs]);
  }

  /** Converts the commit history to be a full clone. */
  gitFetchUnshallow(remote: string) {
    return this.runCommandWithOutput(["git", "fetch", remote, "--unshallow"]);
  }

  /** Fetches the commit history up until a specified revision. */
  gitFetchUntil(remote: string, revision: string) {
    return this.runCommandWithOutput([
      "git",
      "fetch",
      remote,
      `--shallow-exclude=${revision}`,
    ]);
  }

  async gitIsShallow() {
    const output = await this.runCommand([
      "git",
      "rev-parse",
      `--is-shallow-repository`,
    ]);
    return output.trim() === "true";
  }

  /** Fetches from the provided remote. */
  async gitFetchHistory(
    remote: string,
    revision?: string,
  ) {
    if (await this.gitIsShallow()) {
      // only fetch what is necessary
      if (revision != null) {
        await this.gitFetchUntil(remote, revision);
      } else {
        await this.gitFetchUnshallow(remote);
      }
    } else {
      const args = ["git", "fetch", remote, "--recurse-submodules=no"];
      if (revision != null) {
        args.push(revision);
      }
      await this.runCommandWithOutput(args);
    }
  }

  gitFetchTags(remote: string) {
    return this.runCommandWithOutput([
      "git",
      "fetch",
      remote,
      "--tags",
      "--recurse-submodules=no",
    ]);
  }

  async getGitLogFromTags(
    remote: string,
    tagNameFrom: string | undefined,
    tagNameTo: string | undefined,
  ) {
    if (tagNameFrom == null && tagNameTo == null) {
      throw new Error(
        "You must at least supply a tag name from or tag name to.",
      );
    }

    // Ensure we have the git history up to this tag
    // For example, GitHub actions will do a shallow clone.
    try {
      await this.gitFetchHistory(remote, tagNameFrom);
    } catch (err) {
      console.log(`Error fetching commit history: ${err}`);
    }

    // the output of git log is not stable, so use rev-list
    const revs = (await this.runCommand([
      "git",
      "rev-list",
      tagNameFrom == null ? tagNameTo! : `${tagNameFrom}..${tagNameTo ?? ""}`,
    ])).split(/\r?\n/).filter((r) => r.trim().length > 0);

    const lines = await Promise.all(revs.map((rev) => {
      return this.runCommand([
        "git",
        "log",
        "--format=%s",
        "-n",
        "1",
        rev,
      ]).then((message) => ({
        rev,
        message: message.trim(),
      }));
    }));

    return new GitLogOutput(lines);
  }

  /** Gets the git remotes where the key is the remote name and the value is the url. */
  async getGitRemotes() {
    const remotesText = await this.runCommand(["git", "remote"]);
    const remoteNames = remotesText.split(/\r?\n/)
      .filter((l) => l.trim().length > 0);
    const remotes: { [name: string]: string } = {};
    for (const name of remoteNames) {
      remotes[name] =
        (await this.runCommand(["git", "remote", "get-url", name])).trim();
    }
    return remotes;
  }

  /** Gets the commit message for the current commit. */
  async gitCurrentCommitMessage() {
    return (await this.runCommand([
      "git",
      "log",
      "-1",
      "--pretty=%B",
    ])).trim();
  }

  /** Gets the latest tag on the current branch. */
  async gitLatestTag() {
    return (await this.runCommand([
      "git",
      "describe",
      "--tags",
      "--abbrev=0",
    ])).trim();
  }

  async getGitTags() {
    return new GitTags((await this.runCommand(["git", "tag"])).split(/\r?\n/));
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
}

export function getCratesPublishOrder(crates: Iterable<Crate>) {
  const pendingCrates = [...crates];
  const sortedCrates = [];

  while (pendingCrates.length > 0) {
    for (let i = pendingCrates.length - 1; i >= 0; i--) {
      const crate = pendingCrates[i];
      const hasPendingDependency = crate.descendantDependenciesInRepo()
        .some((c) => pendingCrates.includes(c));
      if (!hasPendingDependency) {
        sortedCrates.push(crate);
        pendingCrates.splice(i, 1);
      }
    }
  }

  return sortedCrates;
}
