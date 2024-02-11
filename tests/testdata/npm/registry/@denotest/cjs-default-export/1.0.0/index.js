Object.defineProperty(module.exports, "__esModule", {
  value: true
});
module.exports["default"] = function() {
  return 1;
};
module.exports["named"] = function() {
  return 2;
};

class MyClass {
  static someStaticMethod() {
    return "static method";
  }
}

module.exports.MyClass = MyClass;
