fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/ip.c");
    #[cfg(not(target_os = "windows"))]
    {
        cc::Build::new()
            .file("src/ip.c")
            .compile("ip");
    }
}