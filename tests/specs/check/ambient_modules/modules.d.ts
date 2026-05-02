declare module "*.svg" {
  const content: string;
  export default content;
}
declare module "*.module.css" {
  const classes: { readonly [key: string]: string };
  export default classes;
}
declare module "*.css" {}
