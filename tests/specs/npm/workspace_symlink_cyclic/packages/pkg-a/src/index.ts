import { packageAFromPackageB } from "pkg-b";

export function packageA() {
  console.info("invoked pkg-a.packageA");
  packageAFromPackageB();
}

export function packageBFromPackageA() {
  console.info("invoked packageBFromPackageA");
}
