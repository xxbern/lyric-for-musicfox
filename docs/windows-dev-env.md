# Windows 开发环境

更新：2026-08-26。Windows 构建必须走 MSVC 工具链：skia 预编译库仅提供
`x86_64-pc-windows-msvc` 目标，Skia C++ 无法在 gnu 工具链下构建。

## 环境组成

| 组件 | 安装 |
|---|---|
| rustup-msvc | `scoop install rustup-msvc` |
| VS Build Tools 2022（C++ 桌面负载） | `winget install Microsoft.VisualStudio.2022.BuildTools --source winget --override "--add Microsoft.VisualStudio.Workload.VCTools --includeRecommended --passive"` |

验证：`rustc --version` 显示 msvc host；`where.exe cargo.exe` 可定位。

## 项目布局

唯一工作副本在 Windows 本地盘，WSL 经 /mnt/c 访问：

```
C:\Users\xx\Projects\lyric-for-musicfox
```

禁止在 `\\wsl.localhost\...` 路径下用 cargo.exe 构建（9P IO 慢，
Skia 包解压直接失败）。

## 构建与运行

```bash
# WSL：
cd /mnt/c/Users/xx/Projects/lyric-for-musicfox
./build.sh windows     # release + skia → target/x86_64-pc-windows-msvc/release/
```

cargo.exe 探测顺序：PATH → scoop/shims → apps/rustup-msvc/.cargo/bin，
均失败可设 `CARGO_EXE=<路径>`。

常见问题：

```bash
# no default toolchain configured
export RUSTUP_HOME=/mnt/c/Users/xx/scoop/persist/rustup-msvc/.rustup
export CARGO_HOME=/mnt/c/Users/xx/scoop/persist/rustup-msvc/.cargo

# GitHub 下载 Skia 预编译包被重置
export HTTPS_PROXY=http://127.0.0.1:7890   # 按实际端口
```

运行时：

```bat
set SLINT_BACKEND=winit-skia-software   # 缺省 femtovg(GL)，内存不达标
set LFM_MEM_DIAG=1                      # 生成 mem_diag.csv
```

验收基准（commit 稳态）：≤40MB，实测 26.5MB。
