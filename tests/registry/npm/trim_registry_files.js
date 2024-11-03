#!/usr/bin/env -S deno run --allow-write=. --allow-read=.
// Copyright 2018-2024 the Deno authors. All rights reserved. MIT license.

// Run this to trim the registry.json files

for (const dirPath of getPackageDirs()) {
  if (dirPath.includes("@denotest")) {
    continue;
  }
  const versions = Array.from(Deno.readDirSync(dirPath)
    .map(e => extractVersion(e.name)))
    .filter(e => e != null);

  const registryPath = dirPath + "/registry.json";
  const data = JSON.parse(Deno.readTextFileSync(registryPath));
  // this is to save data
  delete data.readme;
  for (const version in data.versions) {
    if (!versions.includes(version)) {
      delete data.versions[version];
    } else {
      delete data._id;
      delete data._rev;
      delete data.users;
      delete data.contributors;
      delete data.maintainers;
      delete data.keywords;
      delete data.time;
      delete data.versions[version].contributors;
      delete data.versions[version].homepage;
      delete data.versions[version].keywords;
      delete data.versions[version].maintainers;
      delete data.versions[version]._npmUser;
      delete data.versions[version]._npmOperationalInternal;
      delete data.versions[version].dist.signatures;
      delete data.versions[version].dist["npm-signature"];
      if (!versions.includes(data["dist-tags"].latest)) {
        data["dist-tags"].latest = [...versions].sort().pop();
      }
      for (const distTag in data["dist-tags"]) {
        if (!versions.includes(data["dist-tags"][distTag])) {
          delete data["dist-tags"][distTag];
        }
      }
    }
  }
  Deno.writeTextFileSync(registryPath, JSON.stringify(data, null, 2) + "\n");
}

function extractVersion(name) {
  const index = name.lastIndexOf('-');
  if (index === -1)
    return undefined;
  return name.substring(index + 1).replace(/\.tgz$/, "");
}

function* getPackageDirs() {
  for (const entry of Deno.readDirSync(import.meta.dirname)) {
    if (!entry.isDirectory) {
      continue;
    }

    if (entry.name.startsWith("@")) {
      const dirPath = import.meta.dirname + "/" + entry.name;
      for (const entry of Deno.readDirSync(dirPath)) {
        if (!entry.isDirectory) {
          continue;
        }
        yield dirPath + "/" + entry.name;
      }
    } else {
      yield import.meta.dirname + "/" + entry.name;
    }
  }
}
