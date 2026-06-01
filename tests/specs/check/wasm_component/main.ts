import { encode, getUser } from "./component.wasm";

const result = getUser("alice");
const displayName: number = result.tag === "found"
  ? result.val.displayName
  : "missing";
encode("not bytes");
