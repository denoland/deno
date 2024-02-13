#!/usr/bin/env -S deno run --allow-net --allow-read --allow-write
// Copyright 2018-2023 the Deno authors. All rights reserved. MIT license.
// Ported from unicode_width rust crate, Copyright (c) 2015 The Rust Project Developers. MIT license.

import { assert } from "../../assert/assert.ts";
import { runLengthEncode } from "../_rle.ts";

// change this line and re-run the script to update for new Unicode versions
const UNICODE_VERSION = "15.0.0";

const NUM_CODEPOINTS = 0x110000;
const MAX_CODEPOINT_BITS = Math.ceil(Math.log2(NUM_CODEPOINTS - 1));

const OffsetType = {
  U2: 2,
  U4: 4,
  U8: 8,
} as const;

type OffsetType = typeof OffsetType[keyof typeof OffsetType];

type CodePoint = number;
type BitPos = number;

const TABLE_CFGS: [BitPos, BitPos, OffsetType][] = [
  [13, MAX_CODEPOINT_BITS, OffsetType.U8],
  [6, 13, OffsetType.U8],
  [0, 6, OffsetType.U2],
];

async function fetchUnicodeData(filename: string, version: string) {
  const res = await fetch(
    `https://www.unicode.org/Public/${version}/ucd/${filename}`,
  );

  if (!res.ok) {
    throw new Error(`Failed to fetch ${filename}`);
  }

  return await res.text();
}

const EffectiveWidth = {
  Zero: 0,
  Narrow: 1,
  Wide: 2,
  Ambiguous: 3,
} as const;

type EffectiveWidth = typeof EffectiveWidth[keyof typeof EffectiveWidth];

const widthCodes = {
  N: EffectiveWidth.Narrow,
  Na: EffectiveWidth.Narrow,
  H: EffectiveWidth.Narrow,
  W: EffectiveWidth.Wide,
  F: EffectiveWidth.Wide,
  A: EffectiveWidth.Ambiguous,
};

async function loadEastAsianWidths(version: string) {
  const eaw = await fetchUnicodeData("EastAsianWidth.txt", version);

  const single = /^([0-9A-F]+);(\w+)/;
  const multiple = /^([0-9A-F]+)\.\.([0-9A-F]+);(\w+)/;

  const widthMap: EffectiveWidth[] = [];
  let current = 0;

  for (const line of eaw.split("\n")) {
    let rawData: [string, string, string] | null = null;

    let match: RegExpMatchArray | null = null;
    // deno-lint-ignore no-cond-assign
    if (match = line.match(single)) {
      rawData = [match[1], match[1], match[2]];
      // deno-lint-ignore no-cond-assign
    } else if (match = line.match(multiple)) {
      rawData = [match[1], match[2], match[3]];
    } else {
      continue;
    }

    const low = parseInt(rawData[0], 16);
    const high = parseInt(rawData[1], 16);
    const width = widthCodes[rawData[2] as keyof typeof widthCodes];

    assert(current <= high);

    while (current <= high) {
      widthMap.push(current < low ? EffectiveWidth.Narrow : width);
      ++current;
    }
  }

  while (widthMap.length < NUM_CODEPOINTS) {
    widthMap.push(EffectiveWidth.Narrow);
  }

  return widthMap;
}

async function loadZeroWidths(version: string) {
  const categories = await fetchUnicodeData("UnicodeData.txt", version);

  const zwMap: boolean[] = [];
  let current = 0;

  for (const line of categories.split("\n")) {
    const rawData = line.split(";");

    if (rawData.length !== 15) {
      continue;
    }
    const [codepoint, name, catCode] = [
      parseInt(rawData[0], 16),
      rawData[1],
      rawData[2],
    ];

    const zeroWidth = ["Cc", "Cf", "Mn", "Me"].includes(catCode);

    assert(current <= codepoint);

    while (current <= codepoint) {
      if (name.endsWith(", Last>") || (current === codepoint)) {
        zwMap.push(zeroWidth);
      } else {
        zwMap.push(false);
      }
      ++current;
    }
  }
  while (zwMap.length < NUM_CODEPOINTS) {
    zwMap.push(false);
  }

  return zwMap;
}

class Bucket {
  entrySet: Set<string>;
  widths: EffectiveWidth[];

  constructor() {
    this.entrySet = new Set();
    this.widths = [];
  }

  append(codepoint: CodePoint, width: EffectiveWidth) {
    this.entrySet.add(JSON.stringify([codepoint, width]));
    this.widths.push(width);
  }

  tryExtend(attempt: Bucket) {
    const [less, more] = [this.widths, attempt.widths].sort((a, b) =>
      a.length - b.length
    );

    if (!more.slice(0, less.length).every((v, i) => v === less[i])) {
      return false;
    }

    for (const x of attempt.entrySet.values()) {
      this.entrySet.add(x);
    }

    this.widths = more;

    return true;
  }

  entries() {
    const result = [...this.entrySet]
      .map((x) => JSON.parse(x) as [CodePoint, EffectiveWidth]);

    return result.sort((a, b) => a[0] - b[0]);
  }

  width() {
    return new Set(this.widths).size === 1 ? this.widths[0] : null;
  }
}

function makeBuckets(
  entries: [CodePoint, EffectiveWidth][],
  lowBit: BitPos,
  capBit: BitPos,
) {
  const numBits = capBit - lowBit;
  assert(numBits > 0);
  const buckets = Array.from({ length: 2 ** numBits }, () => new Bucket());

  const mask = (1 << numBits) - 1;

  for (const [codepoint, width] of entries) {
    buckets[(codepoint >> lowBit) & mask].append(codepoint, width);
  }

  return buckets;
}

class Table {
  lowBit: BitPos;
  capBit: BitPos;
  offsetType: OffsetType;
  entries: number[];
  indexed: Bucket[];

  constructor(
    entryGroups: [CodePoint, EffectiveWidth][][],
    lowBit: BitPos,
    capBit: BitPos,
    offsetType: OffsetType,
  ) {
    this.lowBit = lowBit;
    this.capBit = capBit;
    this.offsetType = offsetType;
    this.entries = [];
    this.indexed = [];

    const buckets = entryGroups.flatMap((entries) =>
      makeBuckets(entries, this.lowBit, this.capBit)
    );

    for (const bucket of buckets) {
      let extended = false;
      for (const [i, existing] of this.indexed.entries()) {
        if (existing.tryExtend(bucket)) {
          this.entries.push(i);
          extended = true;
          break;
        }
      }
      if (!extended) {
        this.entries.push(this.indexed.length);
        this.indexed.push(bucket);
      }
    }

    for (const index of this.entries) {
      assert(index < (1 << this.offsetType));
    }
  }

  indicesToWidths() {
    if (!this.indexed) {
      throw new Error(`Can't call indicesToWidths twice on the same Table`);
    }

    this.entries = this.entries.map((i) => {
      const width = this.indexed[i].width();
      if (width === null) throw new TypeError("width cannot be null");
      return width;
    });

    this.indexed = null as unknown as Bucket[];
  }

  get buckets() {
    if (!this.indexed) {
      throw new Error(`Can't access buckets after calling indicesToWidths`);
    }

    return this.indexed;
  }

  toBytes() {
    const entriesPerByte = Math.trunc(8 / this.offsetType);
    const byteArray: number[] = [];
    for (let i = 0; i < this.entries.length; i += entriesPerByte) {
      let byte = 0;
      for (let j = 0; j < entriesPerByte; ++j) {
        byte |= this.entries[i + j] << (j * this.offsetType);
      }
      byteArray.push(byte);
    }

    return byteArray;
  }
}

function makeTables(
  tableCfgs: [BitPos, BitPos, OffsetType][],
  entries: [CodePoint, EffectiveWidth][],
) {
  const tables: Table[] = [];
  let entryGroups = [entries];

  for (const [lowBit, capBit, offsetType] of tableCfgs) {
    const table = new Table(entryGroups, lowBit, capBit, offsetType);
    entryGroups = table.buckets.map((bucket) => bucket.entries());

    tables.push(table);
  }

  return tables;
}

export async function tables(version: string) {
  console.info(`Generating tables for Unicode ${version}`);

  const eawMap = await loadEastAsianWidths(version);
  const zwMap = await loadZeroWidths(version);

  const widthMap = eawMap.map((x, i) => zwMap[i] ? EffectiveWidth.Zero : x);

  widthMap[0x00AD] = EffectiveWidth.Narrow;

  for (let i = 0x1160; i < 0x11FF + 1; ++i) {
    widthMap[i] = EffectiveWidth.Zero;
  }

  const tables = makeTables(TABLE_CFGS, [...widthMap.entries()]);

  tables[tables.length - 1].indicesToWidths();

  return tables;
}

const data = {
  UNICODE_VERSION,
  tables: (await tables(UNICODE_VERSION)).map((table) =>
    runLengthEncode(table.toBytes())
  ),
};

assert(data.UNICODE_VERSION.split(".").length === 3);
assert(data.tables.length === 3);

await Deno.writeTextFile("../_data.json", JSON.stringify(data, null, 2) + "\n");
