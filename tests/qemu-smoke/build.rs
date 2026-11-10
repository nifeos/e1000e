//! Link the program with `link.ld`, which places it at one megabyte and gives QEMU the note that
//! names its 32-bit entry point.

fn main() {
    let directory = std::env::var("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR");
    println!("cargo:rerun-if-changed=link.ld");
    println!("cargo:rustc-link-arg-bins=-T{directory}/link.ld");
}
