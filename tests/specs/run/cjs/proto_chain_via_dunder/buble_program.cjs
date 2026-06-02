// deno-lint-ignore-file no-var
// Regression test for https://github.com/denoland/deno/issues/24413
//
// Buble's `Program` extends a base `Node` class whose prototype exposes
// an `initialise(...)` method. The inheritance is wired with
// `Program.prototype.__proto__ = Node.prototype`. With
// `Object.prototype.__proto__` deleted at bootstrap, that assignment
// became a silent own-property write that left the real [[Prototype]]
// unchanged, so `this.body.initialise(...)` inside the constructor
// surfaced as "TypeError: this.body.initialise is not a function".
function Node() {
  this.parent = null;
}
Node.prototype.initialise = function initialise(scope) {
  this.initialised = true;
  this.scope = scope;
};

function BlockStatement() {
  Node.call(this);
}
BlockStatement.prototype.__proto__ = Node.prototype;

function Program() {
  Node.call(this);
  this.body = new BlockStatement();
  // This is the call that throws in buble's Program ctor without the fix.
  this.body.initialise("module");
}
Program.prototype.__proto__ = Node.prototype;

var p = new Program();
console.log("body.initialised:", p.body.initialised);
console.log("body.scope:", p.body.scope);
console.log("body instanceof Node:", p.body instanceof Node);
console.log("program instanceof Node:", p instanceof Node);
