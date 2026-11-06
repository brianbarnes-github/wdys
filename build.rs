//! Embeds the application icon into wdys.exe when building for Windows.

fn main() {
    println!("cargo:rerun-if-changed=resources/wdys.rc");
    println!("cargo:rerun-if-changed=resources/wdys.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        embed_resource::compile("resources/wdys.rc", embed_resource::NONE)
            .manifest_optional()
            .expect("could not embed the application icon");
    }
}
