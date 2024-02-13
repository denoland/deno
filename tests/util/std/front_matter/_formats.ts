// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.

type Delimiter = string | [begin: string, end: string];

/** @deprecated (will be removed after 1.0.0) Use literal types `"yaml" | "toml" | "json" | "unknown"`. */
export enum Format {
  YAML = "yaml",
  TOML = "toml",
  JSON = "json",
  UNKNOWN = "unknown",
}

const { isArray } = Array;

function getBeginToken(delimiter: Delimiter): string {
  return isArray(delimiter) ? delimiter[0] : delimiter;
}

function getEndToken(delimiter: Delimiter): string {
  return isArray(delimiter) ? delimiter[1] : delimiter;
}

function createRegExp(...dv: Delimiter[]): [RegExp, RegExp] {
  const beginPattern = "(" + dv.map(getBeginToken).join("|") + ")";
  const pattern = "^(" +
    "\\ufeff?" + // Maybe byte order mark
    beginPattern +
    "$([\\s\\S]+?)" +
    "^(?:" + dv.map(getEndToken).join("|") + ")\\s*" +
    "$" +
    (globalThis?.Deno?.build?.os === "windows" ? "\\r?" : "") +
    "(?:\\n)?)";

  return [
    new RegExp("^" + beginPattern + "$", "im"),
    new RegExp(pattern, "im"),
  ];
}

const [RX_RECOGNIZE_YAML, RX_YAML] = createRegExp(
  ["---yaml", "---"],
  "= yaml =",
  "---",
);
const [RX_RECOGNIZE_TOML, RX_TOML] = createRegExp(
  ["---toml", "---"],
  "\\+\\+\\+",
  "= toml =",
);
const [RX_RECOGNIZE_JSON, RX_JSON] = createRegExp(
  ["---json", "---"],
  "= json =",
);

export const MAP_FORMAT_TO_RECOGNIZER_RX = {
  yaml: RX_RECOGNIZE_YAML,
  toml: RX_RECOGNIZE_TOML,
  json: RX_RECOGNIZE_JSON,
} as const;

export const MAP_FORMAT_TO_EXTRACTOR_RX = {
  yaml: RX_YAML,
  toml: RX_TOML,
  json: RX_JSON,
} as const;
