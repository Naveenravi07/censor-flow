
fn main() {
    let lib_path = "lib/vosk"; 
    println!("cargo:rustc-link-search=native={}", lib_path);
    println!("cargo:rustc-link-lib=dylib=vosk");
}

