#!/bin/bash
# Provisions a machine for this template: Rust (+ wasm target, rustfmt, clippy),
# ALSA headers, wasm-opt, Xvfb, Blender, and GIMP. Built for the Setup script
# field of a Claude Code cloud environment (claude.ai/code -> environment menu
# -> Edit -> Setup script), but it works on any Debian/Ubuntu box. Paste this
# whole file there, or just the one-liner:
#
#   curl -fsSL https://raw.githubusercontent.com/CassidyPrather/game-template/main/scripts/cloud-setup.sh | bash
#
# Blender and GIMP come from their official downloads when the network allows
# it (latest stable release by default). The default "Trusted" network level
# does not include download.blender.org or download.gimp.org, so there the
# script falls back to Ubuntu's older packages. To get the latest builds, set
# the environment's network access to Custom, tick "Also include default list
# of common package managers", and allow both domains.
#
# Knobs, as environment variables (set them in the environment's settings, or
# prefix the one-liner, e.g. `curl ... | BLENDER_VERSION=apt bash`):
#   BLENDER_VERSION  latest (default) | an exact release like 5.2.2 | apt | skip
#   GIMP_VERSION     latest (default) | an exact release like 3.2.6 | apt | skip
#   RUST_TOOLCHAIN   toolchain to install as the default (stable); repos with a
#                    rust-toolchain.toml also get their pinned toolchain
#
# Never fails the session: a setup script that exits non-zero stops the cloud
# session from starting, so every step warns instead, and the summary at the
# end says what did not work. Keep it under ~5 minutes so the environment gets
# cached; on a typical cloud VM it takes about two.
set -uo pipefail

BLENDER_VERSION="${BLENDER_VERSION:-latest}"
GIMP_VERSION="${GIMP_VERSION:-latest}"
RUST_TOOLCHAIN="${RUST_TOOLCHAIN:-stable}"

# Cloud setup scripts run as root. Elsewhere, run as yourself: apt goes
# through sudo, and Rust installs for your user rather than root's.
SUDO=""
if [ "$(id -u)" -ne 0 ]; then
    SUDO="sudo"
fi
export DEBIAN_FRONTEND=noninteractive

FAILED=()
log() { printf '\n==> %s\n' "$*"; }
warn() {
    printf 'cloud-setup: %s\n' "$*" >&2
    FAILED+=("$*")
}

apt_install() {
    $SUDO apt-get install -y --no-install-recommends "$@"
}

# Fetches a URL into a file; quiet on success, short reason on failure.
fetch() {
    curl -fsSL --retry 3 --connect-timeout 15 -o "$2" "$1"
}

# Prints a directory listing's hrefs that match an extended regex, oldest
# version first. Both download sites serve plain autoindex pages.
list_matching() {
    curl -fsSL --retry 3 --connect-timeout 15 "$1" |
        grep -oE "href=\"$2\"" | sed -E 's/^href="(.*)"$/\1/' | sort -uV
}

# Points a command on PATH at an installed binary.
link_bin() {
    $SUDO ln -sfn "$1" "/usr/local/bin/$2"
}

install_system_packages() {
    log "System packages"
    $SUDO apt-get update || warn "apt-get update failed"
    # libasound2-dev: macroquad's native audio links -lasound.
    # binaryen: wasm-opt, which scripts/build-web.sh uses to shrink the wasm.
    # xvfb: a virtual display, for GUI-only operations (xvfb-run blender ...).
    # The X/GL libraries are what the official Blender build expects to find
    # even in background mode; the GIMP AppImage bundles its own.
    apt_install \
        ca-certificates curl git xz-utils build-essential pkg-config \
        libasound2-dev binaryen xvfb \
        libx11-6 libxi6 libxxf86vm1 libxfixes3 libxrender1 libxkbcommon0 \
        libsm6 libice6 libgl1 libegl1 libglu1-mesa \
        || warn "apt-get install of the base packages failed"
}

install_rust() {
    log "Rust"
    if ! command -v rustup >/dev/null 2>&1 && [ -f "$HOME/.cargo/env" ]; then
        # shellcheck disable=SC1091
        . "$HOME/.cargo/env"
    fi
    if ! command -v rustup >/dev/null 2>&1; then
        curl -fsSL --retry 3 https://sh.rustup.rs |
            sh -s -- -y --profile minimal --default-toolchain none ||
            { warn "rustup install failed"; return; }
        # shellcheck disable=SC1091
        . "$HOME/.cargo/env"
    fi

    rustup toolchain install "$RUST_TOOLCHAIN" --profile minimal \
        --component rustfmt --component clippy \
        --target wasm32-unknown-unknown ||
        warn "rustup could not install the $RUST_TOOLCHAIN toolchain"
    rustup default "$RUST_TOOLCHAIN" >/dev/null || true

    # Pre-install the toolchain that each checked-out repo pins, and warm its
    # crate cache, so the first cargo command in a session does not stall.
    # Harmless if no repo has been cloned yet.
    local toml dir seen=" "
    for toml in "${CLAUDE_PROJECT_DIR:-$PWD}/rust-toolchain.toml" /home/*/*/rust-toolchain.toml; do
        [ -f "$toml" ] || continue
        dir="$(cd "$(dirname "$toml")" && pwd -P)"
        case "$seen" in *" $dir "*) continue ;; esac
        seen="$seen$dir "
        (cd "$dir" && rustup toolchain install && cargo fetch --quiet) ||
            warn "could not prepare the pinned toolchain in $dir"
    done
}

# Official Linux build from download.blender.org into /opt/blender-<version>.
install_blender_official() {
    [ "$(uname -m)" = "x86_64" ] || { echo "No official Linux build for $(uname -m)"; return 1; }
    local base="https://download.blender.org/release" series tarball="" version
    if [ "$BLENDER_VERSION" = "latest" ]; then
        # Newest series that has a Linux build (a brand-new series can be
        # listed before its first release lands).
        for series in $(list_matching "$base/" 'Blender[0-9]+\.[0-9]+/' | tac); do
            tarball="$(list_matching "$base/$series" 'blender-[0-9]+\.[0-9]+\.[0-9]+-linux-x64\.tar\.xz' | tail -n 1)"
            [ -n "$tarball" ] && break
        done
    else
        series="Blender${BLENDER_VERSION%.*}/"
        tarball="blender-${BLENDER_VERSION}-linux-x64.tar.xz"
    fi
    [ -n "$tarball" ] || return 1
    version="${tarball#blender-}"
    version="${version%-linux-x64.tar.xz}"

    if [ -x "/opt/blender-$version/blender" ]; then
        echo "Blender $version already installed"
    else
        local tmp
        tmp="$(mktemp -d)"
        echo "Downloading $base/$series$tarball"
        fetch "$base/$series$tarball" "$tmp/$tarball" &&
            fetch "$base/${series}blender-$version.sha256" "$tmp/sums" &&
            (cd "$tmp" && grep -F " $tarball" sums | sha256sum -c -) &&
            $SUDO mkdir -p "/opt/blender-$version" &&
            $SUDO tar -xJf "$tmp/$tarball" -C "/opt/blender-$version" --strip-components=1
        local status=$?
        rm -rf "$tmp"
        [ "$status" -eq 0 ] || { $SUDO rm -rf "/opt/blender-$version"; return 1; }
    fi
    link_bin "/opt/blender-$version/blender" blender
}

install_blender() {
    log "Blender ($BLENDER_VERSION)"
    case "$BLENDER_VERSION" in
        skip) return ;;
        apt) ;;
        *)
            install_blender_official && return
            echo "Official Blender build unavailable; falling back to Ubuntu's package"
            ;;
    esac
    apt_install blender || warn "Blender install failed"
}

# Official AppImage from download.gimp.org, unpacked into /opt/gimp-<version>.
# Unpacked rather than run directly because AppImages need FUSE, which
# containers usually lack.
install_gimp_official() {
    local arch base="https://download.gimp.org/gimp" series="" image="" version
    arch="$(uname -m)"
    if [ "$GIMP_VERSION" = "latest" ]; then
        for series in $(list_matching "$base/" 'v[0-9]+\.[0-9]+/' | tac); do
            image="$(list_matching "$base/${series}linux/" "GIMP-[0-9]+\.[0-9]+\.[0-9]+-$arch\.AppImage" | tail -n 1)"
            [ -n "$image" ] && break
        done
    else
        series="v${GIMP_VERSION%.*}/"
        image="GIMP-${GIMP_VERSION}-$arch.AppImage"
    fi
    [ -n "$image" ] || return 1
    version="${image#GIMP-}"
    version="${version%-"$arch".AppImage}"

    if [ -x "/opt/gimp-$version/AppRun" ]; then
        echo "GIMP $version already installed"
    else
        local tmp
        tmp="$(mktemp -d)"
        echo "Downloading $base/${series}linux/$image"
        fetch "$base/${series}linux/$image" "$tmp/$image" &&
            fetch "$base/${series}linux/SHA256SUMS" "$tmp/sums" &&
            (cd "$tmp" && grep -F " $image" sums | sha256sum -c -) &&
            chmod +x "$tmp/$image" &&
            (cd "$tmp" && "./$image" --appimage-extract >/dev/null) &&
            $SUDO rm -rf "/opt/gimp-$version" &&
            $SUDO mv "$tmp/squashfs-root" "/opt/gimp-$version"
        local status=$?
        rm -rf "$tmp"
        [ "$status" -eq 0 ] || return 1
        # Skip the startup check for newer versions; it cannot reach gimp.org
        # from a sandbox and prints a warning on every batch run.
        local rc
        for rc in "/opt/gimp-$version"/usr/etc/gimp/*/gimprc; do
            [ -f "$rc" ] && echo '(check-updates no)' | $SUDO tee -a "$rc" >/dev/null
        done
    fi
    # AppRun finds its own directory through the symlink, so this is enough.
    link_bin "/opt/gimp-$version/AppRun" gimp
}

install_gimp() {
    log "GIMP ($GIMP_VERSION)"
    case "$GIMP_VERSION" in
        skip) return ;;
        apt) ;;
        *)
            install_gimp_official && return
            echo "Official GIMP build unavailable; falling back to Ubuntu's package"
            ;;
    esac
    apt_install gimp || warn "GIMP install failed"
}

report() {
    log "Installed"
    command -v rustc >/dev/null && rustc --version
    command -v wasm-opt >/dev/null && wasm-opt --version
    command -v blender >/dev/null && blender --background --factory-startup --version 2>/dev/null | head -n 1
    command -v gimp >/dev/null && gimp --version 2>/dev/null | tail -n 1
    if [ "${#FAILED[@]}" -gt 0 ]; then
        printf '\nSome steps failed; the session will still start:\n' >&2
        printf '  - %s\n' "${FAILED[@]}" >&2
    fi
}

install_system_packages
install_rust
install_blender
install_gimp
report
exit 0
