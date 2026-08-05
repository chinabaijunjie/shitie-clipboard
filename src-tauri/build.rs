fn main() {
    // macOS Tahoe 26 + cargo 增量构建兼容性修复：
    // tauri-build 把 ../resources/* 复制到 target/{profile}/ 下的 resources 副本目录，
    // 复制产生的目标文件被 macOS 打上 com.apple.provenance 标记。增量构建时，
    // fs::copy 覆盖带 provenance 的既有文件会触发 Permission denied（TCC 拦截）。
    //
    // 副本落点因模式不同（实测）：
    // - debug（tauri dev）：target/debug/_up_/resources/（_up_ 是 tauri-utils resource_relpath 把 ../ 转换）
    // - release（tauri build）：target/release/resources/（无 _up_ 前缀，直接落 profile 根下）
    //
    // 修复：构建前先清理两种路径下的旧 resources 副本，让每次复制都是"新建"而非"覆盖"。
    if let Ok(out_dir) = std::env::var("OUT_DIR") {
        if let Some(target_profile) = std::path::Path::new(&out_dir)
            .parent()
            .and_then(|p| p.parent())
            .and_then(|p| p.parent())
        {
            // debug 模式：清理 target/{profile}/_up_（resource_relpath ../ 转换结果）
            let stale_up = target_profile.join("_up_");
            if stale_up.exists() {
                let _ = std::fs::remove_dir_all(&stale_up);
            }
            // release 模式：清理 target/{profile}/resources（tauri-build 直接落点，无 _up_ 前缀）
            let stale_resources = target_profile.join("resources");
            if stale_resources.exists() {
                let _ = std::fs::remove_dir_all(&stale_resources);
            }
        }
    }

    tauri_build::build()
}
