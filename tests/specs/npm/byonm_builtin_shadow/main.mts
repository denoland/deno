import events from "events";
// The "events" specifier must resolve to the Node.js built-in, not the
// shadowing npm package in node_modules — matching Node.js's resolution.
console.log(events.FROM_NPM === true ? "shadowed" : "builtin");
