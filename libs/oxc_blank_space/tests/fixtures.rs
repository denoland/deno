// Integration tests using ts-blank-space test fixtures.
// See: https://github.com/bloomberg/ts-blank-space

use oxc_blank_space::blank_space;

fn fixture_test(name: &str) {
    let input_path = format!(
        "{}/tests/fixture/cases/{}.ts",
        env!("CARGO_MANIFEST_DIR"),
        name
    );
    let output_path = format!(
        "{}/tests/fixture/output/{}.js",
        env!("CARGO_MANIFEST_DIR"),
        name
    );

    let input = std::fs::read_to_string(&input_path)
        .unwrap_or_else(|e| panic!("Failed to read {input_path}: {e}"));
    let expected = std::fs::read_to_string(&output_path)
        .unwrap_or_else(|e| panic!("Failed to read {output_path}: {e}"));

    match blank_space(&input) {
        Ok(actual) => {
            if actual != expected {
                // Find first difference for a helpful message
                let actual_lines: Vec<&str> = actual.lines().collect();
                let expected_lines: Vec<&str> = expected.lines().collect();
                let mut diff_msg = String::new();
                let max_lines = actual_lines.len().max(expected_lines.len());
                for i in 0..max_lines {
                    let a = actual_lines.get(i).unwrap_or(&"<EOF>");
                    let e = expected_lines.get(i).unwrap_or(&"<EOF>");
                    if a != e {
                        diff_msg.push_str(&format!(
                            "\n  First difference at line {}:\n    expected: {:?}\n    actual:   {:?}\n",
                            i + 1,
                            e,
                            a,
                        ));
                        // Show a few more lines of context
                        for j in (i + 1)..((i + 4).min(max_lines)) {
                            let a2 = actual_lines.get(j).unwrap_or(&"<EOF>");
                            let e2 = expected_lines.get(j).unwrap_or(&"<EOF>");
                            if a2 != e2 {
                                diff_msg.push_str(&format!(
                                    "  line {}:\n    expected: {:?}\n    actual:   {:?}\n",
                                    j + 1,
                                    e2,
                                    a2,
                                ));
                            }
                        }
                        break;
                    }
                }
                panic!(
                    "Fixture {name} output mismatch (actual {} bytes, expected {} bytes){diff_msg}",
                    actual.len(),
                    expected.len(),
                );
            }
        }
        Err(errors) => {
            let msgs: Vec<String> = errors.iter().map(|e| e.to_string()).collect();
            panic!(
                "Fixture {name} produced errors:\n{}",
                msgs.join("\n")
            );
        }
    }
}

#[test]
fn fixture_a() {
    fixture_test("a");
}

#[test]
fn fixture_b() {
    fixture_test("b");
}

#[test]
fn fixture_arrow_functions() {
    fixture_test("arrow-functions");
}

#[test]
fn fixture_modules() {
    fixture_test("modules");
}

#[test]
fn fixture_asi() {
    fixture_test("asi");
}

#[test]
fn fixture_decorators() {
    fixture_test("decorators");
}

#[test]
fn fixture_namespaces() {
    fixture_test("namespaces");
}

#[test]
fn fixture_parenthetised_types() {
    fixture_test("parenthetised-types");
}
