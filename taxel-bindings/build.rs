use std::{
    env, io,
    path::{Path, PathBuf},
};

fn main() -> io::Result<()> {
    let cargo_manifest_dir =
        env::var("CARGO_MANIFEST_DIR").expect("Missing environment variable 'CARGO_MANIFEST_DIR'");
    let cargo_manifest_dir = Path::new(&cargo_manifest_dir).join("..");

    let library_path =
        env::var("LIBRARY_PATH").expect("Missing environment variable 'LIBRARY_PATH'");
    let library_name =
        env::var("LIBRARY_NAME").expect("Missing environment variable 'LIBRARY_NAME'");
    let header_file = env::var("HEADER_FILE").expect("Missing environment variable 'HEADER_FILE'");

    let library_path = cargo_manifest_dir.join(library_path);
    let header_file = cargo_manifest_dir.join(header_file);

    println!("cargo:rustc-link-search={}", library_path.display());
    println!("cargo:rustc-link-lib={}", library_name);
    println!("cargo:rerun-if-changed={}", header_file.display());
    println!("cargo:rustc-env=LD_LIBRARY_PATH={}", library_path.display());

    let header = header_file.to_str().expect("Can't convert path to string");

    let bindings = bindgen::Builder::default()
        .header(header)
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
