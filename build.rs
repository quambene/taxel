use bindgen;
use std::{env, io, path::PathBuf};

fn main() -> io::Result<()> {
    let library_path =
        env::var("LIBRARY_PATH").expect("Missing environment variable 'LIBRARY_PATH'");
    let library_name =
        env::var("LIBRARY_NAME").expect("Missing environment variable 'LIBRARY_NAME'");
    let header_file = env::var("HEADER_FILE").expect("Missing environment variable 'HEADER_FILE'");

    println!("cargo:rustc-link-search={}", library_path);
    println!("cargo:rustc-link-lib={}", library_name);
    println!("cargo:rerun-if-changed={}", header_file);

    let bindings = bindgen::Builder::default()
        .header(header_file)
        .parse_callbacks(Box::new(bindgen::CargoCallbacks))
        .generate()
        .expect("Can't generate bindings");

    let out_dir = env::var("OUT_DIR").expect("Can't read environment variable 'OUT_DIR'");
    let output_path = PathBuf::from(out_dir);

    bindings
        .write_to_file(output_path.join("bindings.rs"))
        .expect("Can't write bindings");

    Ok(())
}
