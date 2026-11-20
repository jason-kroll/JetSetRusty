use std::process::Command;

fn main() {
    // No C is compiled by this crate — the whole game is Rust. SDL3 is linked
    // by the sdl3 crate's bundled build (sdl3-sys compiles SDL3 from source and
    // emits the link directives itself), so there is no system SDL package to
    // probe for anymore.

    // Datestamped version string, exposed to Rust as env!("BUILD") and shown on
    // the loader screen.
    let date = Command::new("date")
        .arg("+%y.%m.%d")
        .output()
        .expect("failed to run date!")
        .stdout;
    let date = String::from_utf8(date)
        .expect("date output is UTF-8")
        .trim()
        .to_string();
    println!("cargo:rustc-env=BUILD=1.0.0 {date}");
}
