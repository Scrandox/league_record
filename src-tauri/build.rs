fn main() {
    fetch_ffmpeg();
    build_helper::Builder::new().with_path("./target/").build().unwrap();
    tauri_build::build();
}

/// Auto-Clip bundles ffmpeg (see 'resources' in tauri.conf.json), which is too big to keep in
/// git. Fetch it on the first build of a fresh checkout so 'bun run tauri build' just works.
fn fetch_ffmpeg() {
    use std::path::Path;
    use std::process::Command;

    println!("cargo:rerun-if-changed=bin/ffmpeg.exe");
    println!("cargo:rerun-if-changed=../scripts/fetch-ffmpeg.ps1");

    if Path::new("bin/ffmpeg.exe").is_file() {
        return;
    }

    println!("cargo:warning=bin/ffmpeg.exe is missing - running scripts/fetch-ffmpeg.ps1");

    let status = Command::new("pwsh")
        .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File", "../scripts/fetch-ffmpeg.ps1"])
        .status();

    match status {
        Ok(status) if status.success() => {}
        Ok(status) => panic!("scripts/fetch-ffmpeg.ps1 failed with {status} - run it by hand to see why"),
        Err(e) => panic!("failed to run scripts/fetch-ffmpeg.ps1 ({e}) - fetch ffmpeg by hand into src-tauri/bin/"),
    }
}
