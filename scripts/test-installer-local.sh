#!/bin/sh
set -eu

repo_root="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
installer="${INSTALLER_SCRIPT:-$repo_root/scripts/claude-md-symlinker-installer.sh}"
app_name="claude-md-symlinker"
root="$(mktemp -d)"
trap 'rm -rf "$root"' EXIT HUP INT TERM

say() {
    printf '%s\n' "$*"
}

die() {
    printf 'error: %s\n' "$*" >&2
    exit 1
}

target_triple() {
    os="${CLAUDE_MD_SYMLINKER_OS:-$(uname -s)}"
    arch="${CLAUDE_MD_SYMLINKER_ARCH:-$(uname -m)}"
    case "$os:$arch" in
        Darwin:arm64|Darwin:aarch64)
            printf 'aarch64-apple-darwin\n'
            ;;
        Darwin:x86_64|Darwin:amd64)
            printf 'x86_64-apple-darwin\n'
            ;;
        Linux:arm64|Linux:aarch64)
            printf 'aarch64-unknown-linux-gnu\n'
            ;;
        Linux:x86_64|Linux:amd64)
            printf 'x86_64-unknown-linux-gnu\n'
            ;;
        *)
            die "unsupported test platform: $os $arch"
            ;;
    esac
}

checksum_file() {
    archive="$1"
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$archive" | awk '{print $1}'
    else
        shasum -a 256 "$archive" | awk '{print $1}'
    fi
}

make_fixture_release() {
    target="$1"
    fixture="$2"
    archive="$app_name-$target.tar.xz"

    cargo build --quiet --manifest-path "$repo_root/Cargo.toml"
    mkdir -p "$fixture/pkg/$app_name-$target"
    cp "$repo_root/target/debug/$app_name" "$fixture/pkg/$app_name-$target/$app_name"
    (
        cd "$fixture/pkg"
        tar -cJf "$fixture/$archive" "$app_name-$target"
    )
    digest="$(checksum_file "$fixture/$archive")"
    {
        printf '%s  %s\n' "$digest" "$archive"
        printf '\n'
    } > "$fixture/$archive.sha256"
}

make_fake_curl() {
    fakebin="$1"
    fixture="$2"
    mkdir -p "$fakebin"
    cat > "$fakebin/curl" <<'EOF'
#!/bin/sh
set -eu
output=""
url=""
while [ "$#" -gt 0 ]; do
    case "$1" in
        -o)
            output="$2"
            shift 2
            ;;
        -*)
            shift
            ;;
        *)
            url="$1"
            shift
            ;;
    esac
done
[ -n "$output" ] || exit 2
[ -n "$url" ] || exit 2
name="${url##*/}"
cp "$INSTALLER_TEST_FIXTURE/$name" "$output"
EOF
    chmod +x "$fakebin/curl"
}

assert_contains() {
    file="$1"
    needle="$2"
    grep -F "$needle" "$file" >/dev/null 2>&1 || {
        printf 'missing expected text in %s: %s\n' "$file" "$needle" >&2
        printf '%s\n' "--- $file ---" >&2
        cat "$file" >&2
        exit 1
    }
}

assert_not_contains() {
    file="$1"
    needle="$2"
    if grep -F "$needle" "$file" >/dev/null 2>&1; then
        printf 'unexpected text in %s: %s\n' "$file" "$needle" >&2
        printf '%s\n' "--- $file ---" >&2
        cat "$file" >&2
        exit 1
    fi
}

run_installer_case() {
    name="$1"
    home="$2"
    install_dir="$3"
    path_value="$4"
    output="$root/$name.out"

    say "testing installer: $name"
    env \
        HOME="$home" \
        PATH="$path_value" \
        CLAUDE_MD_SYMLINKER_INSTALL_DIR="$install_dir" \
        CLAUDE_MD_SYMLINKER_DATA_DIR="$home/data" \
        CLAUDE_MD_SYMLINKER_NO_PROGRESS=1 \
        CLAUDE_MD_SYMLINKER_AUTO_MIGRATE=1 \
        CLAUDE_MD_SYMLINKER_NO_SERVICE=1 \
        INSTALLER_TEST_FIXTURE="$fixture" \
        sh "$installer" >"$output" 2>&1

    assert_contains "$output" "Installed $app_name to $install_dir/$app_name"
    assert_contains "$output" "installed Claude hooks"
    assert_contains "$output" "Auto migrate: enabled."
    assert_not_contains "$output" "improperly formatted"
    assert_contains "$home/.claude/settings.json" "claude-md-symlinker managed hook v1"
}

target="$(target_triple)"
fixture="${INSTALLER_FIXTURE_DIR:-$root/release}"
fakebin="$root/fakebin"
mkdir -p "$fixture"

if [ -z "${INSTALLER_FIXTURE_DIR:-}" ]; then
    make_fixture_release "$target" "$fixture"
fi
archive="$fixture/$app_name-$target.tar.xz"
[ -f "$archive" ] || die "missing fixture archive: $archive"
[ -f "$archive.sha256" ] || die "missing fixture checksum: $archive.sha256"

make_fake_curl "$fakebin" "$fixture"

home_with_path="$root/home-with-path"
install_with_path="$root/bin-with-path"
mkdir -p "$home_with_path" "$install_with_path"
run_installer_case \
    "bin-already-in-path" \
    "$home_with_path" \
    "$install_with_path" \
    "$install_with_path:$fakebin:$PATH"

home_without_parent="$root/home-without-parent"
install_without_path="$root/bin-without-path"
mkdir -p "$install_without_path"
run_installer_case \
    "profile-parent-missing" \
    "$home_without_parent" \
    "$install_without_path" \
    "$fakebin:$PATH"

say "installer local tests passed"
