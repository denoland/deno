module.exports = {
  getValue() {
    return this.otherMethod();
  },
  otherMethod() {
    return 1;
  },
};
