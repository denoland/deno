// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
import db from "./vendor/mime-db.v1.52.0.ts";
import { type DBEntry, extensions } from "./_util.ts";

export type KeyOfDb = keyof typeof db;

/** A map of the media type for a given extension */
export const types = new Map<string, KeyOfDb>();

/** Internal function to populate the maps based on the Mime DB. */
(function populateMaps() {
  const preference = ["nginx", "apache", undefined, "iana"];

  for (const type of Object.keys(db) as KeyOfDb[]) {
    const mime = db[type] as DBEntry;
    const exts = mime.extensions;

    if (!exts || !exts.length) {
      continue;
    }

    // @ts-ignore work around denoland/dnt#148
    extensions.set(type, exts);

    for (const ext of exts) {
      const current = types.get(ext);
      if (current) {
        const from = preference.indexOf((db[current] as DBEntry).source);
        const to = preference.indexOf(mime.source);

        if (
          current !== "application/octet-stream" &&
          (from > to ||
            // @ts-ignore work around denoland/dnt#148
            (from === to && current.startsWith("application/")))
        ) {
          continue;
        }
      }

      types.set(ext, type);
    }
  }
})();

export { db };
