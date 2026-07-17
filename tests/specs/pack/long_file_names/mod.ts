// The import path is intentionally long: once prefixed with `package/` and emitted
// as `.js`/`.d.ts` it exceeds the 100 byte GNU tar `name` field, which is the whole
// point of this test. Don't shorten it. https://github.com/denoland/deno/issues/36008
export { schemaName } from "./kubernetes/models/_schemas/IoK8sApiAdmissionregistrationV1beta1ValidatingAdmissionPolicyBindingList.ts";
export type { ValidatingAdmissionPolicyBindingList } from "./kubernetes/models/_schemas/IoK8sApiAdmissionregistrationV1beta1ValidatingAdmissionPolicyBindingList.ts";
