import test from "@denotest/cjs-default-export";

export default function() {
  return test.default() * 5;
}
