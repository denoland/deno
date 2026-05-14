// Read results.jsonl + micro_results.jsonl and print a markdown ratios table
// (Deno vs Node, Deno vs Bun) per test. Used to populate the PR report body.
//
// Usage:  node analyze.js
import fs from "node:fs";

function read(path) {
  if (!fs.existsSync(path)) return [];
  return fs.readFileSync(path, "utf8")
    .split("\n")
    .filter(Boolean)
    .map((l) => JSON.parse(l));
}

function fmt(n) {
  if (n === undefined || n === null || !isFinite(n)) return "—";
  if (Math.abs(n) >= 100) return n.toFixed(1);
  if (Math.abs(n) >= 10) return n.toFixed(2);
  return n.toFixed(3);
}

function ratioTable(rows, idKey, valueKey, runtimeKey, label, lowerIsBetter) {
  // rows: list of {<runtimeKey>: "deno"|"node"|"bun", <idKey>, <valueKey>: number}
  const byId = new Map();
  for (const r of rows) {
    const id = r[idKey];
    if (!byId.has(id)) byId.set(id, {});
    byId.get(id)[r[runtimeKey]] = Number(r[valueKey]);
  }
  const out = [];
  out.push(`| ${label} | Deno | Node | Bun | Deno/Node | Deno/Bun |`);
  out.push(`| --- | ---: | ---: | ---: | ---: | ---: |`);
  for (const [id, vals] of byId) {
    const d = vals.deno, n = vals.node, b = vals.bun;
    const dn = d && n ? (lowerIsBetter ? d / n : n / d) : undefined;
    const db = d && b ? (lowerIsBetter ? d / b : b / d) : undefined;
    out.push(
      `| ${id} | ${fmt(d)} | ${fmt(n)} | ${fmt(b)} | ${fmt(dn)} | ${fmt(db)} |`,
    );
  }
  return out.join("\n");
}

const servers = read("results.jsonl");
const micros = read("micro_results.jsonl");

console.log("## HTTP server (rps; higher is better) — ratios show Node/Deno and Bun/Deno (>1 = competitor faster)\n");

// servers entries have label like "deno_hello", "node_hello", etc. Split out runtime.
const sRows = servers.map((r) => {
  const [rt, ...rest] = r.label.split("_");
  return { runtime: rt, route: rest.join("_"), rps: Number(r.rps), lat_p99: r.lat_p99 };
});
console.log(ratioTable(sRows, "route", "rps", "runtime", "route", /*lowerIsBetter*/ false));

console.log("\n## Microbench (ns/op; lower is better) — ratios show Deno/Node and Deno/Bun (>1 = Deno slower)\n");
console.log(ratioTable(micros, "name", "ns_per_op", "runtime", "op", /*lowerIsBetter*/ true));
