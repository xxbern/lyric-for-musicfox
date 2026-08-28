fn main() {
    // material 风格（设置窗 widgets）；lyric.slint 不用 widget 集，不受影响
    let config = slint_build::CompilerConfiguration::new().with_style("material".into());
    slint_build::compile_with_config("ui/lib.slint", config).unwrap();

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let _ = std::fs::metadata("assets/icon.ico").expect("missing assets/icon.ico");

        // 从 Cargo.toml 自动获取版本号，填充生成 app.manifest 与 icon.rc
        let pkg_version = std::env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "0.0.0".into());
        let manifest_template = std::fs::read_to_string("assets/app.manifest")
            .expect("missing assets/app.manifest");
        let manifest_content = manifest_template.replace("0.0.0.0", &format!("{pkg_version}.0"));

        let out_dir = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
        let manifest_path = out_dir.join("app.manifest");
        std::fs::write(&manifest_path, manifest_content).unwrap();

        let icon_path = std::path::Path::new("assets/icon.ico")
            .canonicalize()
            .unwrap();
        let rc_content = format!(
            "1 ICON \"{}\"\n1 24 \"{}\"\n",
            icon_path.display().to_string().replace('\\', "/"),
            manifest_path.display().to_string().replace('\\', "/")
        );
        let rc_path = out_dir.join("icon.rc");
        std::fs::write(&rc_path, rc_content).unwrap();

        embed_resource::compile(&rc_path, embed_resource::NONE)
            .manifest_optional()
            .unwrap();
    }
}
