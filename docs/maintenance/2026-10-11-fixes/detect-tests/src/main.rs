fn main() {
    let Some(root) = std::env::args().nth(1) else {
        eprintln!("Usage: quickdev-isolated-detect-tests <project-directory>");
        std::process::exit(2);
    };
    match quickdev_isolated_detect_tests::detect::detect_project(&root) {
        Ok(result) => println!("{}", serde_json::to_string_pretty(&result).unwrap()),
        Err(error) => { eprintln!("{error}"); std::process::exit(1); }
    }
}