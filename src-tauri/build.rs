use std::path::PathBuf;

fn main() {
    tauri_build::build();

    // ---- 编译期：告诉链接器去哪找 libopenblt.so 并链接它 ----
    // CARGO_MANIFEST_DIR 就是 src-tauri/ 目录（libopenblt.so 就在这里）
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    println!("cargo:rustc-link-search=native={}", manifest_dir);
    // libopenblt.so -> 库名写成 "openblt"（lib 前缀和 .so 后缀省略）
    println!("cargo:rustc-link-lib=openblt");

    // ---- 运行期：让二进制能找到 .so ----
    // 1) 把 .so 复制到二进制同目录 target/<profile>，配合下面的 $ORIGIN rpath
    if let Ok(out_dir) = std::env::var("OUT_DIR") {
        // OUT_DIR = <target>/<profile>/build/<pkg>-<hash>/out  → 上三级即 <target>/<profile>
        if let Ok(profile_dir) = PathBuf::from(&out_dir).join("../../..").canonicalize() {
            let _ = std::fs::copy(
                PathBuf::from(manifest_dir).join("libopenblt.so"),
                profile_dir.join("libopenblt.so"),
            );
        }
    }
    // 2) 设置 rpath：先在二进制同目录($ORIGIN)找，再回退到 src-tauri 绝对目录找
    println!("cargo:rustc-link-arg=-Wl,-rpath,$ORIGIN");
    println!("cargo:rustc-link-arg=-Wl,-rpath,{}", manifest_dir);
}
