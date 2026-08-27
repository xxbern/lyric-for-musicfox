#!/usr/bin/env bash
#
# build.sh - compile helper for lyric-for-musicfox (Rust + Slint)
#
# Supports:
#   - Native debug / release builds for Linux and Windows.
#   - Windows builds invoke cargo.exe (MSVC toolchain) directly:
#     required by the skia renderer, whose prebuilt binaries only
#     exist for x86_64-pc-windows-msvc. Run the script from a path
#     on a Windows-local disk (/mnt/c/... under WSL interop).
#   - Optional build of the patched go-musicfox companion via the
#     existing musicfox-patch/build_musicfox_with_plugin.sh.
#
# Usage:
#   ./build.sh                      # native debug build
#   ./build.sh release              # native release build (default)
#   ./build.sh debug
#   ./build.sh windows              # Windows build via cargo.exe (release, +skia)
#   ./build.sh windows-debug        # Windows build via cargo.exe (debug)
#   ./build.sh musicfox             # also build patched go-musicfox
#   ./build.sh clean                # cargo clean
#   ./build.sh check                # cargo check (no codegen)
#   ./build.sh test                 # cargo test
#
# Environment variables (override defaults):
#   PROFILE       "release" | "debug"             (default: release)
#   TARGET_LINUX  rust target triple             (default: x86_64-unknown-linux-gnu)
#   TARGET_WIN    rust target triple             (default: x86_64-pc-windows-msvc)
#   WIN_FEATURES  features enabled on Windows    (default: skia; set empty to disable)
#   JOBS          parallel jobs passed to cargo   (default: nproc)
#   EXTRA_CARGO   extra flags passed to cargo
#
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "${SCRIPT_DIR}"

# ----- defaults ---------------------------------------------------------
PROFILE="${PROFILE:-release}"
TARGET_LINUX="${TARGET_LINUX:-x86_64-unknown-linux-gnu}"
TARGET_WIN="${TARGET_WIN:-x86_64-pc-windows-msvc}"
WIN_FEATURES="${WIN_FEATURES-skia}"
JOBS="${JOBS:-}"
EXTRA_CARGO="${EXTRA_CARGO:-}"

# Pre-built binary path used by release runs.
BIN_NAME="lyric-for-musicfox"
TARGET_DIR="${SCRIPT_DIR}/target"

# ----- helpers ----------------------------------------------------------
log()  { printf '\033[1;34m[build.sh]\033[0m %s\n' "$*"; }
warn() { printf '\033[1;33m[build.sh]\033[0m %s\n' "$*" >&2; }
die()  { printf '\033[1;31m[build.sh]\033[0m %s\n' "$*" >&2; exit 1; }

require() {
    command -v "$1" >/dev/null 2>&1 || die "required tool '$1' not found in PATH"
}

jobs_flag() {
    if [[ -n "${JOBS}" ]]; then
        printf -- "-j%s" "${JOBS}"
    elif command -v nproc >/dev/null 2>&1; then
        printf -- "-j%s" "$(nproc)"
    fi
}

# ----- build steps ------------------------------------------------------
build_rust() {
    local target="$1" profile="$2"
    log "Building Rust binary"
    log "  target : ${target}"
    log "  profile: ${profile}"

    require cargo

    local cargo_args=(build)
    if [[ "${profile}" == "release" ]]; then
        cargo_args+=(--release)
        local out="${TARGET_DIR}/${target}/release/${BIN_NAME}.exe"
        [[ "${target}" != *windows* && -z "${out##*${BIN_NAME}}" ]] && out="${TARGET_DIR}/${target}/release/${BIN_NAME}"
    else
        local out="${TARGET_DIR}/${target}/debug/${BIN_NAME}.exe"
        [[ "${target}" != *windows* && -z "${out##*${BIN_NAME}}" ]] && out="${TARGET_DIR}/${target}/debug/${BIN_NAME}"
    fi

    # shellcheck disable=SC2206
    local cargo_cmd=(cargo "${cargo_args[@]}" --target "${target}" $(jobs_flag) ${EXTRA_CARGO})

    log "  cmd    : ${cargo_cmd[*]}"
    "${cargo_cmd[@]}"

    if [[ -x "${TARGET_DIR}/${target}/${profile}/${BIN_NAME}" ]]; then
        log "  built  : ${TARGET_DIR}/${target}/${profile}/${BIN_NAME}"
    fi
}

ensure_target_installed() {
    local target="$1"
    if ! rustup target list --installed 2>/dev/null | grep -q "^${target}\$"; then
        log "Installing rustup target: ${target}"
        rustup target add "${target}"
    fi
}

build_linux_native() {
    local profile="$1"
    build_rust "${TARGET_LINUX}" "${profile}"
}

build_windows_native() {
    local profile="$1"
    local cargo_exe="${CARGO_EXE:-cargo.exe}"

    require "${cargo_exe}"

    local feat_args=()
    [[ -n "${WIN_FEATURES}" ]] && feat_args+=(--features "${WIN_FEATURES}")

    log "Building Windows binary via ${cargo_exe} (MSVC)"
    log "  target   : ${TARGET_WIN}"
    log "  profile  : ${profile}"
    log "  features : ${WIN_FEATURES:-<none>}"

    # shellcheck disable=SC2086
    "${cargo_exe}" build \
        $( [[ "${profile}" == "release" ]] && printf -- "--release" ) \
        --target "${TARGET_WIN}" \
        ${feat_args[@]+"${feat_args[@]}"} \
        $(jobs_flag) ${EXTRA_CARGO}

    local out="${TARGET_DIR}/${TARGET_WIN}/${profile}/lyric-for-musicfox.exe"
    [[ -f "${out}" ]] && log "  built  : ${out}"
}

build_musicfox() {
    local script="${SCRIPT_DIR}/musicfox-patch/build_musicfox_with_plugin.sh"
    if [[ ! -x "${script}" ]]; then
        warn "musicfox-patch/build_musicfox_with_plugin.sh not found or not executable; skipping"
        return 0
    fi
    if [[ ! -d "${SCRIPT_DIR}/musicfox-patch/go-musicfox" ]]; then
        warn "musicfox-patch/go-musicfox/ not present; clone go-musicfox next to it before running"
        warn "  git clone https://github.com/go-musicfox/go-musicfox musicfox-patch/go-musicfox"
        return 0
    fi
    require go
    log "Building patched go-musicfox"
    "${script}"
}

do_clean() {
    log "cargo clean"
    cargo clean
}

do_check() {
    require cargo
    log "cargo check"
    cargo check $(jobs_flag) ${EXTRA_CARGO}
}

do_test() {
    require cargo
    log "cargo test"
    cargo test $(jobs_flag) ${EXTRA_CARGO}
}

# ----- dispatch ---------------------------------------------------------
cmd="${1:-release}"
shift || true

case "${cmd}" in
    release|debug)
        PROFILE="${cmd}"
        if [[ "${OSTYPE:-}" == "linux-gnu"* ]]; then
            build_linux_native "${PROFILE}"
        elif [[ "${OSTYPE:-}" == "msys" || "${OSTYPE:-}" == "cygwin" || "${OSTYPE:-}" == "win32" ]]; then
            build_windows_native "${PROFILE}"
        else
            build_linux_native "${PROFILE}"
        fi
        ;;
    linux)
        build_linux_native "${PROFILE}"
        ;;
    windows|windows-release)
        PROFILE="release"
        build_windows_native "${PROFILE}"
        ;;
    windows-debug)
        PROFILE="debug"
        build_windows_native "${PROFILE}"
        ;;
    musicfox)
        build_musicfox
        ;;
    clean)
        do_clean
        ;;
    check)
        do_check
        ;;
    test)
        do_test
        ;;
    all)
        build_linux_native release
        build_windows_native release
        build_musicfox
        ;;
    help|-h|--help)
        sed -n '2,40p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
        ;;
    *)
        die "unknown command: ${cmd} (run: $0 help)"
        ;;
esac

log "Done."
