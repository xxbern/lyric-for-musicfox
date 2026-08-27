fn main() {
    // material 风格（设置窗 widgets）；lyric.slint 不用 widget 集，不受影响
    let config = slint_build::CompilerConfiguration::new().with_style("material".into());
    slint_build::compile_with_config("ui/lib.slint", config).unwrap();

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let _ = std::fs::metadata("assets/icon.ico").expect("missing assets/icon.ico");

        // 将 assets/icon.rc（声明 `1 ICON "icon.ico"`）编译并链接进 exe，
        // 运行时可通过 tray_icon::Icon::from_resource(1, None) 加载。
        embed_resource::compile("assets/icon.rc", embed_resource::NONE)
            .manifest_optional()
            .unwrap();
    }
}
