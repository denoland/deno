// CVE-2025-55182 in isolation: model key parsing with no `.prototype.then`
// thenable pattern anywhere in the module, so only the stage 1 (RCE) patch
// should apply. This guards against the stage 1 result being dropped when
// stage 2 (the DoS patch) makes no change.
export function parseModelKeys(str) {
  return str.split(":"); // resolved_model key parsing in the flight client; trailing text keeps this line long enough that the matcher's 100-char look-ahead window has content past the split token, mirroring real minified builds.
}
