// intentional type checking errors
export class Class1 extends Class2 {
}

export class Class2 extends Class1 {
}

// these should be fine though
export { subDir } from "./sub_dir";
export { otherDir } from "./other_dir";
