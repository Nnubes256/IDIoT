use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=www/");

    let _ = assert!(Command::new("yarn")
        .args(&["install"])
        .current_dir("www")
        .status()
        .expect("Successful exit")
        .success());

    let _ = assert!(Command::new("yarn")
        .args(&["build"])
        .current_dir("www")
        .status()
        .expect("Successful exit")
        .success());
}
