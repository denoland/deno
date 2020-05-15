import { DispatchJsonCoreOp } from "../dispatch_json.js";

class Counter {
  constructor(start) {
    this.rid = jsonOps["new_counter"].dispatchSync({ start }).rid;
  }

  count(step = 1) {
    return jsonOps["count"].dispatchSync({ rid: this.rid, step }).count;
  }
}

class JsonError extends Error {
  constructor(msg) {
    super(msg);
    this.name = "JsonError";
  }
}

class BadResourceId extends Error {
  constructor(msg) {
    super(msg);
    this.name = "BadResourceId";
  }
}

function errorFactory(kind, msg) {
  switch (kind) {
    case 1:
      return new JsonError(msg);
    case 2:
      return new BadResourceId(msg);
    default:
      return new Error(msg);
  }
}

let jsonOps;
let ops;

function main() {
  ops = Deno.core.ops();
  jsonOps = {};
  for (const opName in ops) {
    jsonOps[opName] = new DispatchJsonCoreOp(ops[opName], errorFactory);
  }
  let count = 5;
  const counter = new Counter(count);
  for (const _key of Array(20).keys()) {
    Deno.core.print(count + "\n");
    count = counter.count(count);
  }
  try {
    jsonOps["new_counter"].dispatchSync({});
  } catch (e) {
    if (!e instanceof JsonError) {
      Deno.core.print(e);
    }
  }
  try {
    jsonOps["count"].dispatchSync({ rid: 25, step: 15 });
  } catch (e) {
    if (!e instanceof BadResourceId) {
      Deno.core.print(e);
    }
  }
}

main();
