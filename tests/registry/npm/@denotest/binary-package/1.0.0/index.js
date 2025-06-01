const packageByOs = {
  "darwin": "@denotest/binary-package-mac",
  "linux": "@denotest/binary-package-linux",
  "win32": "@denotest/binary-package-windows",
}

const selectedPackage = packageByOs[process.platform];

if (!selectedPackage) {
  throw new Error("trying to run on unsupported platform");
}

require(selectedPackage);