function test() {
  // this comment should be in output
  return 1 + 1;
}

// should include the comments because people rely on this behavior
console.log(test.toString());
