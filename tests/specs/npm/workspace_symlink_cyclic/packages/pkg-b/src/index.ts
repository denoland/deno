import { packageBFromPackageA } from "pkg-a";

export function packageB() {
  console.info("invoked pkg-b.packageB");
  packageBFromPackageA();
}

export function packageAFromPackageB() {
  console.info("invoked packageAFromPackageB");
}
