#!/bin/sh
# Install the analysis tools the checks depend on. Safe to run repeatedly:
# anything already present at the pinned version is left alone.
#
#   scripts/install-tools.sh            tools `make check` needs
#   scripts/install-tools.sh coverage   plus cargo-llvm-cov
#   scripts/install-tools.sh mutants    plus cargo-mutants
#   scripts/install-tools.sh fuzz       plus cargo-fuzz
#   scripts/install-tools.sh deep       all of the above
#
# Cargo tools are built from source with `cargo install --locked`, so their
# integrity rests on crates.io checksums. The first build of each takes a few
# minutes; after that this script is instant.
set -eu

CARGO_DENY_VERSION=0.22.1
CARGO_LLVM_COV_VERSION=0.9.2
CARGO_MUTANTS_VERSION=27.1.0
CARGO_FUZZ_VERSION=0.13.2

# install_cargo_tool <crate> <version> <binary> <subcommand>
install_cargo_tool() {
    crate=$1
    version=$2
    binary=$3
    subcommand=$4
    if command -v "$binary" >/dev/null 2>&1 &&
        "$binary" "$subcommand" --version 2>/dev/null | grep -q "$version"; then
        return 0
    fi
    echo "install-tools: building $crate $version (a few minutes, once)"
    cargo install --locked --version "$version" "$crate"
}

install_shellcheck() {
    if command -v shellcheck >/dev/null 2>&1; then
        return 0
    fi
    echo "install-tools: installing shellcheck"
    if command -v brew >/dev/null 2>&1; then
        brew install shellcheck
    elif command -v apt-get >/dev/null 2>&1 && [ "$(id -u)" -eq 0 ]; then
        apt-get update -qq && apt-get install -y -qq shellcheck
    elif command -v apt-get >/dev/null 2>&1 && sudo -n true 2>/dev/null; then
        sudo apt-get update -qq && sudo apt-get install -y -qq shellcheck
    else
        echo "install-tools: shellcheck is missing and could not be installed automatically." >&2
        echo "Install it once, by hand: https://github.com/koalaman/shellcheck#installing" >&2
        return 1
    fi
}

install_cargo_tool cargo-deny "$CARGO_DENY_VERSION" cargo-deny deny
install_shellcheck

for extra in "$@"; do
    case "$extra" in
        coverage | deep)
            install_cargo_tool cargo-llvm-cov "$CARGO_LLVM_COV_VERSION" cargo-llvm-cov llvm-cov
            ;;
    esac
    case "$extra" in
        mutants | deep)
            install_cargo_tool cargo-mutants "$CARGO_MUTANTS_VERSION" cargo-mutants mutants
            ;;
    esac
    case "$extra" in
        fuzz | deep)
            install_cargo_tool cargo-fuzz "$CARGO_FUZZ_VERSION" cargo-fuzz fuzz
            ;;
    esac
    case "$extra" in
        coverage | mutants | fuzz | deep) ;;
        *)
            echo "install-tools: unknown tool set '$extra'" >&2
            exit 2
            ;;
    esac
done

echo "install-tools: ok"
