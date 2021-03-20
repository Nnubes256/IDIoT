extern crate bindgen;

use std::env;
use std::path::PathBuf;

fn main() {
    // Tell cargo to invalidate the built crate whenever the wrapper changes
    println!("cargo:rerun-if-changed=wrapper.h");

    cc::Build::new()
        .files(&[
            "vendor/Raspberry_Pi_2/pi_2_dht_read.c",
            "vendor/Raspberry_Pi_2/pi_2_mmio.c",
            "vendor/common_dht_read.c",
        ])
        .cargo_metadata(true)
        .compile("libadafruitdht11");

    // The bindgen::Builder is the main entry point
    // to bindgen, and lets you build up options for
    // the resulting bindings.
    let bindings = bindgen::Builder::default()
        // The input header we would like to generate
        // bindings for.
        .header("wrapper.h");

    //#[cfg(all(target_arch = "arm", target_os = "linux", target_env = "gnu"))]
    let bindings = bindings.clang_arg("--sysroot=/usr/arm-linux-gnueabi");

    // Tell cargo to invalidate the built crate whenever any of the
    // included header files changed.
    let bindings = bindings
        .parse_callbacks(Box::new(bindgen::CargoCallbacks))
        // Finish the builder and generate the bindings.
        .generate()
        // Unwrap the Result and panic on failure.
        .expect("Unable to generate bindings");

    // Write the bindings to the $OUT_DIR/bindings.rs file.
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");
}
