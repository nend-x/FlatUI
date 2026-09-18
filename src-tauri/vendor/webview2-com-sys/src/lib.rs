#[allow(
    non_snake_case,
    non_upper_case_globals,
    non_camel_case_types,
    dead_code,
    clippy::all
)]
pub mod Microsoft {
    pub mod Web {
        pub mod WebView2 {
            pub mod Win32 {
                mod windows_link {
                    // FlatUI patch (v0.1.5+): link to the SYSTEM WebView2Loader.dll
                    // at runtime via `raw-dylib`, instead of statically embedding
                    // WebView2LoaderStatic.lib into the exe. This:
                    //   - drops the ~150 KB static lib from the binary
                    //   - drops the build-time dependency on the WebView2 SDK
                    //     lib files (no x64/WebView2Loader*.lib needed at all)
                    //   - resolves WebView2Loader.dll from the OS at load time
                    //     (Windows 11 ships it system-wide as part of the
                    //     WebView2 runtime; Win10 needs the Evergreen runtime
                    //     installed, which FlatUI already assumes for the
                    //     Edge-based WebView2 runtime itself)
                    // `raw-dylib` (stable since Rust 1.71) makes rustc emit the
                    // import table directly into the object file, so lld-link
                    // needs no import library on disk. The same name is used on
                    // msvc and gnu targets.
                    //
                    // IMPORTANT: do NOT include the `.dll` extension in `name`
                    // here — `raw-dylib` auto-appends `.dll` on Windows targets,
                    // so writing `WebView2Loader.dll` would emit an import for
                    // `WebView2Loader.dll.dll` (which the loader cannot resolve).
                    // The correct literal is `WebView2Loader` → emitted as
                    // `WebView2Loader.dll`.
                    macro_rules! link_webview2 {
                        ($library:literal $abi:literal fn $($function:tt)*) => (
                            #[link(name = "WebView2Loader", kind = "raw-dylib")]
                            extern $abi {
                                pub fn $($function)*;
                            }
                        )
                    }

                    pub(crate) use link_webview2 as link;
                }

                include!("bindings.rs");
            }
        }
    }
}

pub mod declared_interfaces;

#[cfg(test)]
mod test {
    use windows_core::w;

    use crate::Microsoft::Web::WebView2::Win32::*;

    #[test]
    fn compare_eq() {
        let mut result = 1;
        unsafe { CompareBrowserVersions(w!("1.0.0"), w!("1.0.0"), &mut result) }.unwrap();
        assert_eq!(0, result);
    }

    #[test]
    fn compare_lt() {
        let mut result = 0;
        unsafe { CompareBrowserVersions(w!("1.0.0"), w!("1.0.1"), &mut result) }.unwrap();
        assert_eq!(-1, result);
    }

    #[test]
    fn compare_gt() {
        let mut result = 0;
        unsafe { CompareBrowserVersions(w!("2.0.0"), w!("1.0.1"), &mut result) }.unwrap();
        assert_eq!(1, result);
    }
}
