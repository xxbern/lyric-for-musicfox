#!/usr/bin/env bash
# 动态打补丁并编译 go-musicfox 自动化脚本
# 不污染 go-musicfox 的原生代码仓库，编译完成后自动还原 git 工作区

set -e

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
MUSICFOX_DIR="${SCRIPT_DIR}/go-musicfox"
PATCH_FILE="${SCRIPT_DIR}/lyric_udp.patch"
MUSICFOX_TAG="${MUSICFOX_TAG:-v5.1.0}"

if [ ! -d "${MUSICFOX_DIR}" ]; then
    echo "未找到 go-musicfox 目录，正在从 GitHub 克隆..."
    git clone git@github.com:go-musicfox/go-musicfox.git "${MUSICFOX_DIR}" || \
    git clone https://github.com/go-musicfox/go-musicfox.git "${MUSICFOX_DIR}"
fi

if [ ! -f "${PATCH_FILE}" ]; then
    echo "错误: 未找到补丁文件 ${PATCH_FILE}"
    exit 1
fi

echo "[1/3] 切换至指定版本 ${MUSICFOX_TAG} 并应用 UDP 歌词广播补丁..."
cd "${MUSICFOX_DIR}"
git checkout "${MUSICFOX_TAG}"
git apply "${PATCH_FILE}"

# 捕获退出信号，无论成功或失败均恢复 Git 工作区
cleanup() {
    echo "[3/3] 还原 go-musicfox 代码库到原生干净状态 (git checkout)..."
    git checkout internal/ui/player.go || true
}
trap cleanup EXIT

echo "[2/3] 编译 Windows 64位 go-musicfox (musicfox.exe)..."
# 版本号随 checkout 的 tag 注入（与官方 goreleaser 一致），否则启动会提示发现新版本
APP_VERSION="$(git describe --tags --abbrev=0)"
GOOS=windows GOARCH=amd64 CGO_ENABLED=1 CC=x86_64-w64-mingw32-gcc \
  go build -ldflags "-s -w -X github.com/go-musicfox/go-musicfox/internal/types.AppVersion=${APP_VERSION}" \
  -o musicfox.exe ./cmd/musicfox.go

echo "编译完成！可执行文件已生成在: ${MUSICFOX_DIR}/musicfox.exe"
