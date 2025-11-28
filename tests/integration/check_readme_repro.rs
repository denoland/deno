
use test_util as util;
use util::TestContextBuilder;

#[test]
fn check_jsr_package_readme() {
  let context = TestContextBuilder::new().use_temp_cwd().build();
  let temp_dir = context.temp_dir();
  
  temp_dir.write("deno.json", r#"{
  "name": "@scope/pkg",
  "version": "1.0.0",
  "exports": "./mod.ts"
}"#);
  
  temp_dir.write("mod.ts", "export const a = 1;");
  
  // README with a type error
  temp_dir.write("README.md", r#"
```ts
const a: number = "string";
```
"#);

  let output = context.new_command()
    .args("check .")
    .run();

  // Currently this should succeed (exit code 0) because README is ignored.
  // After fix, it should fail.
  output.assert_exit_code(0);
}
