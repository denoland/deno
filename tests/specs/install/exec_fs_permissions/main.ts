import { getFileMode } from "@denotest/exec-fs-permissions";

// this should output that it has executable permissions
console.log((getFileMode() & 0o777).toString(8));
