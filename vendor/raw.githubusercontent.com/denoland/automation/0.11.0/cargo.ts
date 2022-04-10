// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

import { runCommand } from "./helpers.ts";

export interface CargoMetadata {
  packages: CargoPackageMetadata[];
  /** Identifiers in the `packages` array of the workspace members. */
  "workspace_members": string[];
  /** The absolute workspace root directory path. */
  "workspace_root": string;
}

export interface CargoPackageMetadata {
  id: string;
  name: string;
  version: string;
  dependencies: CargoDependencyMetadata[];
  /** Path to Cargo.toml */
  "manifest_path": string;
}

export interface CargoDependencyMetadata {
  name: string;
  /** Version requrement (ex. ^0.1.0) */
  req: string;
}

export async function getCargoMetadata(directory: string) {
  const result = await runCommand({
    cwd: directory,
    cmd: ["cargo", "metadata", "--format-version", "1"],
  });
  return JSON.parse(result!) as CargoMetadata;
}
