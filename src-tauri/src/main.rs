// FlatUI entry point
// GUI subsystem in release: a shell replacement must never flash a console
// window on launch. (This attribute only has an effect here in the binary
// crate — having it only in lib.rs, as before, silently did nothing.)
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    flatui_lib::run()
}
