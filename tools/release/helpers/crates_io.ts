// Copyright 2018-2022 the Deno authors. All rights reserved. MIT license.

export interface CratesIoMetadata {
  crate: {
    id: string;
    name: string;
  };
  versions: {
    crate: string;
    num: string;
  }[];
}

export async function getCratesIoMetadata(crateName: string) {
  // rate limit
  await new Promise((resolve) => setTimeout(resolve, 100));

  const response = await fetch(`https://crates.io/api/v1/crates/${crateName}`);
  const data = await response.json();

  return data as CratesIoMetadata;
}
