// Copyright 2018-2026 the Deno authors. MIT license.
import { buildPath, existsSync, join } from "./util.js";

const currentDataFile = join(buildPath(), "bench.json");
const allDataFile = "gh-pages/data.json"; // Includes all benchmark data.
const allDataJsonlFile = "gh-pages/data.jsonl"; // JSONL version of all benchmark data.
const recentDataFile = "gh-pages/recent.json"; // Includes recent 20 benchmark data.
const recentDataJsonlFile = "gh-pages/recent.jsonl"; // JSONL version of recent benchmark data.

function readJson(filename) {
  return JSON.parse(Deno.readTextFileSync(filename));
}

function writeJson(filename, data) {
  return Deno.writeTextFileSync(filename, JSON.stringify(data));
}

function writeJsonl(filename, data) {
  return Deno.writeTextFileSync(
    filename,
    data.map((row) => JSON.stringify(row)).join("\n") + "\n",
  );
}

if (!existsSync(currentDataFile)) {
  throw new Error(`${currentDataFile} doesn't exist`);
}

if (!existsSync(allDataFile)) {
  throw new Error(`${allDataFile} doesn't exist`);
}

const newData = readJson(currentDataFile);
const allData = readJson(allDataFile);
allData.push(newData);
const allDataLen = allData.length;
const recentData = allData.slice(allDataLen - 20);

writeJson(allDataFile, allData);
writeJson(allDataJsonlFile, allData);
writeJson(recentDataFile, recentData);
writeJson(recentDataJsonlFile, recentData);
