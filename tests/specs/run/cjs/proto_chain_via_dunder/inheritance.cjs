// Regression test for https://github.com/denoland/deno/issues/34337
// Stylus (and many other npm packages) install the prototype chain via
// `Child.prototype.__proto__ = Parent.prototype`. When Deno deletes
// `Object.prototype.__proto__` for security, that assignment silently
// becomes an own-property write that does *not* update [[Prototype]],
// so calls that rely on the inherited getter (here `nodeName`) fall
// through to the `else` branch and recurse forever.
function Node() {
  this.lineno = 1;
}
Node.prototype = {
  constructor: Node,
  get nodeName() {
    return this.constructor.name.toLowerCase();
  },
};

var Boolean = module.exports = function Boolean(val) {
  Node.call(this);
  if (this.nodeName) {
    this.val = !!val;
  } else {
    // Without the fix, we never set this.val because `nodeName` is
    // undefined (the getter on Node.prototype is unreachable) and we
    // recurse until V8 throws "Maximum call stack size exceeded".
    return new Boolean(val);
  }
};

Boolean.prototype.__proto__ = Node.prototype;

var b = new Boolean(true);
console.log("nodeName:", b.nodeName);
console.log("val:", b.val);
console.log("isNode:", b instanceof Node);
