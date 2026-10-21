# Sourced by scripts/capture_*.sh. A binary older than the sources it's
# meant to show renders yesterday's game, or worse: a shader on disk newer
# than the Rust that feeds it (a uniform grown by a field) fails wgpu's
# validation mid-capture, hours in. Warn before starting.
capture_check_bin() {
    local bin="$1"
    if [ ! -x "$bin" ]; then
        echo "no $bin: run cargo build --profile fast" >&2
        exit 1
    fi
    local newer
    newer="$(find src crates/*/src assets/shaders -newer "$bin" \( -name '*.rs' -o -name '*.wgsl' \) 2>/dev/null | head -3)"
    if [ -n "$newer" ]; then
        echo "warning: $bin is older than the sources (e.g. ${newer//$'\n'/, });" \
            "run cargo build --profile fast first" >&2
    fi
}
