// Released under the MIT License.
// Copyright, 2026, by Samuel Williams.

#[test]
fn lists_shared_and_project_tasks() {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_base-web-multipart-bake"))
        .arg("--list")
        .output()
        .unwrap();
    assert!(output.status.success());
    let tasks = String::from_utf8(output.stdout).unwrap();
    for task in ["test:coverage", "cargo:after_version_bump"] {
        assert!(tasks.contains(task), "{task}");
    }
}
