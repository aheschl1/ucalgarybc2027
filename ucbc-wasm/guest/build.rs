//! Links CPython's WASI build statically, from what `make guest` downloads into
//! `.cache/build`: libpython and the libraries it was built with, and the WASI sysroot.

fn main() {
    let wasi = concat!(env!("CARGO_MANIFEST_DIR"), "/../../.cache/build");
    let python = format!("{wasi}/python-build");
    for dir in [
        python.clone(),
        format!("{python}/Modules/_decimal/libmpdec"),
        format!("{python}/Modules/expat"),
        format!("{python}/Modules/_hacl"),
        format!("{wasi}/wasi-sysroot-24.0/lib/wasm32-wasip1"),
    ] {
        println!("cargo:rustc-link-search=native={dir}");
    }
    for lib in [
        "python3.14",
        "mpdec",
        "expat",
        "Hacl_Hash_MD5",
        "Hacl_Hash_SHA1",
        "Hacl_Hash_SHA2",
        "Hacl_Hash_SHA3",
        "Hacl_Hash_BLAKE2",
        "Hacl_HMAC",
        "dl",
        "wasi-emulated-signal",
        "wasi-emulated-getpid",
        "wasi-emulated-process-clocks",
    ] {
        println!("cargo:rustc-link-lib=static={lib}");
    }
    // A reactor: `_initialize` runs the C constructors once (the snapshot calls it), and
    // exports are not wrapped in constructors and destructors. The stack and initial
    // memory are CPython's own WASI settings.
    println!("cargo:rustc-link-arg={wasi}/wasi-sysroot-24.0/lib/wasm32-wasip1/crt1-reactor.o");
    for arg in [
        "--export=_initialize",
        "-zstack-size=16777216",
        "--stack-first",
        "--initial-memory=41943040",
    ] {
        println!("cargo:rustc-link-arg={arg}");
    }
    println!("cargo:rerun-if-changed={wasi}");
}
