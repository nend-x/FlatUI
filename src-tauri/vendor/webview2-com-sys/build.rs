// FlatUI patch (v0.1.5+): the upstream build.rs copies WebView2Loader.dll,
// WebView2Loader.dll.lib, and WebView2LoaderStatic.lib from x64/x86/arm64
// into OUT_DIR and emits `cargo:rustc-link-search` so lib.rs can find them.
//
// With our raw-dylib patch in src/lib.rs, rustc emits the import table
// inline into the object file — no import library is needed on disk at
// link time, and no static lib is linked. So this build script becomes
// a no-op (we still link advapi32 because bindings.rs calls into it).
//
// The x64/x86/arm64 folders have been removed from the vendored crate.

fn main() {
    // advapi32 is a core Windows system lib; lld-link resolves it from
    // the SDK splat unconditionally.
    println!("cargo:rustc-link-lib=advapi32");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/lib.rs");
}
