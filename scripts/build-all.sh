#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CROSS="$ROOT/scripts/cross"
DIST="${ND_DIST_DIR:-$ROOT/dist}"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT/target-cross}"
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-6}"
NICE="${ND_NICE:-15}"

SWIFTLY_TC="$(command ls -d "$HOME"/.local/share/swiftly/toolchains/*/usr 2>/dev/null | sort -V | tail -1 || true)"
export SDKROOT="${SDKROOT:-$HOME/.swiftpm/swift-sdks/darwin.artifactbundle/Developer/Platforms/MacOSX.platform/Developer/SDKs/MacOSX.sdk}"
export LIBCLANG_PATH="${LIBCLANG_PATH:-${SWIFTLY_TC:+$SWIFTLY_TC/lib}}"
MINGW_BINUTILS="${ND_MINGW_BINUTILS:-$HOME/.local/share/mingw64-binutils/usr/bin}"
ND_PERL="$HOME/.local/share/nv-perl"
if [[ -z "${PERL5LIB:-}" && -d "$ND_PERL" ]]; then
    export PERL5LIB="$ND_PERL/usr/share/perl5:$ND_PERL/usr/share/perl5/vendor_perl:$ND_PERL/usr/lib64/perl5:$ND_PERL/usr/lib64/perl5/vendor_perl"
fi
export PATH="$CROSS/bin:$MINGW_BINUTILS:${SWIFTLY_TC:+$SWIFTLY_TC/bin:}$PATH"
export ND_CROSS_CACHE="$CARGO_TARGET_DIR/cross-cache"

MAC_MIN_X86=10.15
MAC_MIN_ARM=11.0

PKG=nicedb-app
BIN=nicedb
NAME=NiceDB
UPDATER_KEY="${ND_UPDATER_KEY:-$HOME/.config/nicedb/updater.key}"
export ND_COMMIT="${ND_COMMIT:-$(git -C "$ROOT" rev-parse --short=10 HEAD 2>/dev/null || true)}"
export ND_BUILT="${ND_BUILT:-$(git -C "$ROOT" log -1 --format=%cs 2>/dev/null || true)}"

log()  { printf '\n\033[1m==> %s\033[0m\n' "$*"; }
die()  { printf '\033[31merror:\033[0m %s\n' "$*" >&2; exit 1; }
need() { command -v "$1" >/dev/null 2>&1 || die "missing tool: $1 ($2)"; }
size() { du -h "$1" | cut -f1; }
cargo_nice() { nice -n "$NICE" cargo "$@"; }

targets=("$@")
[[ ${#targets[@]} -eq 0 ]] && targets=(linux windows macos)
for t in "${targets[@]}"; do
    case "$t" in linux|windows|macos) ;; *) die "unknown target: $t (linux | windows | macos)";; esac
done

if [[ -z "${ND_ONE:-}" ]]; then
    need cargo "rustup"
    need pnpm "volta/corepack"
    mkdir -p "$DIST" "$ND_CROSS_CACHE/logs"
    if [[ -z "${ND_SKIP_UI:-}" ]]; then
        log "licences: scripts/licenses.mjs"
        node "$ROOT/scripts/licenses.mjs"
        log "UI: pnpm build"
        [[ -d "$ROOT/app/ui/node_modules" ]] || pnpm -C "$ROOT/app/ui" install --frozen-lockfile
        pnpm -C "$ROOT/app/ui" build
    fi
    [[ -f "$ROOT/app/ui/dist/index.html" ]] || die "app/ui/dist/index.html missing"

    ok=() failed=()
    for t in "${targets[@]}"; do
        tlog="$ND_CROSS_CACHE/logs/$t.log"
        set +e
        ND_ONE=1 ND_SKIP_UI=1 bash "${BASH_SOURCE[0]}" "$t" 2>&1 | tee "$tlog"
        status=${PIPESTATUS[0]}
        set -e
        if [[ $status -eq 0 ]]; then ok+=("$t"); else failed+=("$t"); fi
    done

    (cd "$DIST" && shopt -s nullglob && f=("$NAME"-*) && [[ ${#f[@]} -gt 0 ]] && sha256sum "${f[@]}" > SHA256SUMS || true)
    if [[ ${#ok[@]} -gt 0 && -f "$UPDATER_KEY" ]]; then
        log "release: sign the artefacts, write dist/release/latest.json"
        rm -rf "$DIST/release"
        pass_file="${ND_UPDATER_KEY_PASS:-$UPDATER_KEY.pass}"
        TAURI_SIGNING_PRIVATE_KEY_PASSWORD="$(cat "$pass_file" 2>/dev/null || true)" \
            cargo_nice run -q --release -p nd-release -- "$DIST" "$ROOT/app/src-tauri/tauri.conf.json" "$UPDATER_KEY" || failed+=(release)
    elif [[ ${#ok[@]} -gt 0 ]]; then
        printf '\033[33mwarning:\033[0m no updater key at %s: artefacts not signed, no latest.json\n' "$UPDATER_KEY"
    fi
    log "summary"
    for t in "${ok[@]}"; do printf '    \033[32mok\033[0m      %s\n' "$t"; done
    for t in "${failed[@]}"; do
        printf '    \033[31mFAILED\033[0m  %s  (log: %s)\n' "$t" "$ND_CROSS_CACHE/logs/$t.log"
        grep -E '^error|error:|failed to run custom build command' "$ND_CROSS_CACHE/logs/$t.log" | head -5 | sed 's/^/            /'
    done
    command ls -l "$DIST"
    [[ ${#failed[@]} -eq 0 ]]
    exit
fi

[[ ${#targets[@]} -eq 1 ]] || die "ND_ONE runs exactly one target"
T_NAME="${targets[0]}"
want() { [[ "$T_NAME" == "$1" ]]; }
mkdir -p "$DIST" "$ND_CROSS_CACHE"
cd "$ROOT"

if want windows || want macos; then
    need zig "/usr/bin/zig"
    need cargo-zigbuild "cargo install --locked cargo-zigbuild"
    need llvm-objdump "swiftly toolchain or llvm"
fi
if want windows; then
    rustup target list --installed | grep -x x86_64-pc-windows-gnu >/dev/null || die "rustup target add x86_64-pc-windows-gnu"
    need x86_64-w64-mingw32-windres "mingw64-binutils extracted into $MINGW_BINUTILS"
    need x86_64-w64-mingw32-objdump "mingw64-binutils"
    need llvm-ar "swiftly toolchain or llvm"
    need llvm-objcopy "swiftly toolchain or llvm"
    need zip "zip"
    [[ -n "$LIBCLANG_PATH" && -d "$LIBCLANG_PATH" ]] || die "LIBCLANG_PATH: need a libclang matching zig's clang major version"
fi
if want macos; then
    for t in aarch64-apple-darwin x86_64-apple-darwin; do
        rustup target list --installed | grep -x "$t" >/dev/null || die "rustup target add $t"
    done
    [[ -d "$SDKROOT/System/Library/Frameworks/WebKit.framework" ]] || die "SDKROOT is not a macOS SDK: $SDKROOT"
    need llvm-lipo "llvm"
    need rcodesign "cargo install --locked apple-codesign"
    need zip "zip"
fi

if want linux && [[ -z "${ND_LINUX_HOST:-}" ]]; then
    need podman "dnf install podman (rootless)"
    need objdump "binutils"
    CF="$ROOT/scripts/linux-build.Containerfile"
    IMG="localhost/nicedb-linux-build:$(sha256sum "$CF" | cut -c1-12)"
    LTARGET="${ND_LINUX_TARGET_DIR:-$CARGO_TARGET_DIR-glibc}"
    LCACHE="${ND_LINUX_CACHE:-$ROOT/.cache/linux-build}"
    if ! podman image exists "$IMG"; then
        log "Linux: building the container image $IMG"
        nice -n "$NICE" podman build -f "$CF" -t "$IMG" "$ROOT/scripts"
    fi
    mkdir -p "$LTARGET" "$LCACHE/cargo-registry" "$LCACHE/cargo-git" "$LCACHE/xdg"
    log "Linux x86_64 (container $IMG, target dir $LTARGET)"
    nice -n "$NICE" podman run --rm --security-opt label=disable --ulimit core=0 \
        --cpus "$CARGO_BUILD_JOBS" --memory "${ND_LINUX_MEMORY:-20g}" \
        -v "$ROOT:$ROOT" -v "$DIST:$DIST" -v "$LTARGET:$LTARGET" \
        -v "$LCACHE/cargo-registry:/usr/local/cargo/registry" \
        -v "$LCACHE/cargo-git:/usr/local/cargo/git" \
        -v "$LCACHE/xdg:/cache/xdg" \
        -e XDG_CACHE_HOME=/cache/xdg -e ND_LINUX_HOST=1 -e ND_SKIP_UI=1 -e ND_ONE=1 \
        -e CARGO_BUILD_JOBS="$CARGO_BUILD_JOBS" -e ND_NICE="$NICE" -e ND_COMMIT -e ND_BUILT \
        -e CARGO_TARGET_DIR="$LTARGET" -e ND_DIST_DIR="$DIST" \
        -w "$ROOT" "$IMG" bash scripts/build-all.sh linux
    glibc="$(objdump -T "$DIST/$NAME-linux-x86_64" | grep -o 'GLIBC_[0-9.]*' | sort -uV | tail -1)"
    echo "    newest glibc symbol: $glibc"
    [[ "$(printf '%s\n' "$glibc" GLIBC_2.35 | sort -V | tail -1)" == GLIBC_2.35 ]] || die "binary needs $glibc (> 2.35)"
fi

if want linux && [[ -n "${ND_LINUX_HOST:-}" ]]; then
    log "Linux x86_64"
    cargo_nice build --release -p "$PKG" --bin "$BIN"
    cp "$CARGO_TARGET_DIR/release/$BIN" "$DIST/$NAME-linux-x86_64"
    echo "dist/$NAME-linux-x86_64 ($(size "$DIST/$NAME-linux-x86_64"))"
    TGZ_STAGE="$ND_CROSS_CACHE/linux-tgz"
    rm -rf "$TGZ_STAGE" && mkdir -p "$TGZ_STAGE/$NAME-linux-x86_64"
    cp "$CARGO_TARGET_DIR/release/$BIN" "$TGZ_STAGE/$NAME-linux-x86_64/$BIN"
    tar -C "$TGZ_STAGE" --owner=0 --group=0 --numeric-owner -czf "$DIST/$NAME-linux-x86_64.tar.gz" "$NAME-linux-x86_64"
    echo "dist/$NAME-linux-x86_64.tar.gz ($(size "$DIST/$NAME-linux-x86_64.tar.gz"))"

    log "Linux x86_64 AppImage + deb"
    rm -rf "$CARGO_TARGET_DIR/release/bundle/appimage" "$CARGO_TARGET_DIR/release/bundle/deb"
    (cd "$ROOT/app/src-tauri" && NO_STRIP=1 APPIMAGE_EXTRACT_AND_RUN=1 nice -n "$NICE" ../ui/node_modules/.bin/tauri build --bundles appimage,deb \
        --config '{"build":{"beforeBuildCommand":""},"bundle":{"active":true,"targets":["appimage","deb"],"category":"DeveloperTool"}}')
    shopt -s nullglob
    images=("$CARGO_TARGET_DIR"/release/bundle/appimage/*.AppImage)
    debs=("$CARGO_TARGET_DIR"/release/bundle/deb/*.deb)
    shopt -u nullglob
    [[ ${#images[@]} -eq 1 ]] || die "expected one AppImage in $CARGO_TARGET_DIR/release/bundle/appimage"
    [[ ${#debs[@]} -eq 1 ]] || die "expected one .deb in $CARGO_TARGET_DIR/release/bundle/deb"
    cp "${images[0]}" "$DIST/$NAME-linux-x86_64.AppImage"
    chmod 755 "$DIST/$NAME-linux-x86_64.AppImage"
    cp "${debs[0]}" "$DIST/$NAME-linux-x86_64.deb"
    echo "dist/$NAME-linux-x86_64.AppImage ($(size "$DIST/$NAME-linux-x86_64.AppImage"))"
    echo "dist/$NAME-linux-x86_64.deb ($(size "$DIST/$NAME-linux-x86_64.deb"))"
fi

if want windows; then
    log "Windows x86_64 (x86_64-pc-windows-gnu)"
    T=x86_64-pc-windows-gnu
    CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER="$CROSS/zig-mingw-link.sh" \
        cargo_nice zigbuild --release --target "$T" -p "$PKG" --bin "$BIN"
    EXE="$CARGO_TARGET_DIR/$T/release/$BIN.exe"

    log "Windows: import table check"
    pe="$(x86_64-w64-mingw32-objdump -p "$EXE")"
    imports="$(sed -n 's/^\s*DLL Name: //p' <<<"$pe" | sort -fu)"
    echo "$imports" | sed 's/^/    /'
    system_dll='^((api-ms-win-[a-z0-9-]+|advapi32|bcrypt|bcryptprimitives|combase|comctl32|comdlg32|crypt32|d3d11|dxgi|msimg32|ncrypt|powrprof|wtsapi32|mpr|dbghelp|dwmapi|gdi32|imm32|kernel32|ntdll|ole32|oleaut32|pdh|propsys|rstrtmgr|secur32|shcore|shell32|shlwapi|user32|userenv|uxtheme|version|winmm|ws2_32|credui|iphlpapi|netapi32|psapi|rpcrt4|setupapi|winhttp|wininet|odbc32)\.dll|winspool\.drv)$'
    bad="$(echo "$imports" | grep -viE "$system_dll" || true)"
    [[ -z "$bad" ]] || die "non-system DLL imports: $bad"
    grep -q 'Subsystem.*Windows GUI' <<<"$pe" || die "exe is not a GUI-subsystem binary"
    cp "$EXE" "$DIST/$NAME-windows-x86_64.exe"
    echo "dist/$NAME-windows-x86_64.exe ($(size "$DIST/$NAME-windows-x86_64.exe"))"
    rm -f "$DIST/$NAME-windows-x86_64.zip"
    WIN_STAGE="$ND_CROSS_CACHE/windows-zip"
    rm -rf "$WIN_STAGE" && mkdir -p "$WIN_STAGE"
    cp "$EXE" "$WIN_STAGE/$NAME.exe"
    (cd "$WIN_STAGE" && zip -q -X "$DIST/$NAME-windows-x86_64.zip" "$NAME.exe")
    echo "dist/$NAME-windows-x86_64.zip ($(size "$DIST/$NAME-windows-x86_64.zip"))"

    if [[ -x "${ND_NSIS:-$HOME/.local/share/mingw-nsis/usr}/bin/makensis" ]]; then
        log "Windows: NSIS installer"
        rm -rf "$CARGO_TARGET_DIR/$T/release/bundle/nsis"
        (cd "$ROOT/app/src-tauri" && CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER="$CROSS/zig-mingw-link.sh" \
            nice -n "$NICE" ../ui/node_modules/.bin/tauri build --runner cargo-zigbuild --target "$T" --bundles nsis \
            --config '{"build":{"beforeBuildCommand":""},"bundle":{"active":true,"targets":["nsis"],"windows":{"webviewInstallMode":{"type":"downloadBootstrapper","silent":true}}}}')
        shopt -s nullglob
        setups=("$CARGO_TARGET_DIR/$T"/release/bundle/nsis/*-setup.exe)
        shopt -u nullglob
        [[ ${#setups[@]} -eq 1 ]] || die "expected one NSIS installer in $CARGO_TARGET_DIR/$T/release/bundle/nsis"
        cp "${setups[0]}" "$DIST/$NAME-windows-x86_64-setup.exe"
        echo "dist/$NAME-windows-x86_64-setup.exe ($(size "$DIST/$NAME-windows-x86_64-setup.exe"))"
    else
        printf '\033[33mwarning:\033[0m no makensis in %s: NSIS installer skipped\n' "${ND_NSIS:-$HOME/.local/share/mingw-nsis/usr}"
    fi
fi

if want macos; then
    for T in aarch64-apple-darwin x86_64-apple-darwin; do
        log "macOS $T"
        if [[ "$T" == aarch64-* ]]; then min=$MAC_MIN_ARM; else min=$MAC_MIN_X86; fi
        tvar="CARGO_TARGET_$(tr 'a-z-' 'A-Z_' <<<"$T")_RUSTFLAGS"
        lvar="CARGO_TARGET_$(tr 'a-z-' 'A-Z_' <<<"$T")_LINKER"
        env "$tvar=${!tvar:-} -C link-arg=-Wl,-headerpad_max_install_names" \
            "$lvar=$CROSS/zig-macos-link.sh" ND_ZIG_TARGET="${T%%-*}-macos.$min-none" \
            MACOSX_DEPLOYMENT_TARGET="$min" nice -n "$NICE" cargo zigbuild --release --target "$T" -p "$PKG" --bin "$BIN"
    done

    log "macOS: universal binary + $NAME.app"
    read -r IDENT VERSION < <(python3 -c 'import json,sys;c=json.load(open(sys.argv[1]));print(c["identifier"],c["version"])' app/src-tauri/tauri.conf.json)
    STAGE="$ND_CROSS_CACHE/macos"
    APP="$STAGE/$NAME.app"
    rm -rf "$STAGE" && mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
    llvm-lipo -create -output "$APP/Contents/MacOS/$BIN" \
        "$CARGO_TARGET_DIR/aarch64-apple-darwin/release/$BIN" \
        "$CARGO_TARGET_DIR/x86_64-apple-darwin/release/$BIN"
    chmod 755 "$APP/Contents/MacOS/$BIN"
    cp app/src-tauri/icons/icon.icns "$APP/Contents/Resources/icon.icns"
    printf 'APPL????' > "$APP/Contents/PkgInfo"
    cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>CFBundleDevelopmentRegion</key><string>en</string>
	<key>CFBundleDisplayName</key><string>$NAME</string>
	<key>CFBundleName</key><string>$NAME</string>
	<key>CFBundleExecutable</key><string>$BIN</string>
	<key>CFBundleIdentifier</key><string>$IDENT</string>
	<key>CFBundleIconFile</key><string>icon.icns</string>
	<key>CFBundleInfoDictionaryVersion</key><string>6.0</string>
	<key>CFBundlePackageType</key><string>APPL</string>
	<key>CFBundleShortVersionString</key><string>$VERSION</string>
	<key>CFBundleVersion</key><string>$VERSION</string>
	<key>LSApplicationCategoryType</key><string>public.app-category.developer-tools</string>
	<key>LSMinimumSystemVersion</key><string>$MAC_MIN_X86</string>
	<key>LSArchitecturePriority</key><array><string>arm64</string><string>x86_64</string></array>
	<key>NSHighResolutionCapable</key><true/>
	<key>NSSupportsAutomaticGraphicsSwitching</key><true/>
	<key>NSRequiresAquaSystemAppearance</key><false/>
</dict>
</plist>
PLIST

    log "macOS: linked dylibs check"
    dylibs="$(llvm-objdump --macho --dylibs-used --arch=all "$APP/Contents/MacOS/$BIN" | grep -E '^\s+/' | awk '{print $1}' | sort -u)"
    echo "$dylibs" | sed 's/^/    /'
    bad="$(echo "$dylibs" | grep -vE '^/(usr/lib|System/Library)/' || true)"
    [[ -z "$bad" ]] || die "non-system dylibs linked: $bad"

    log "macOS: ad-hoc signature (rcodesign)"
    rcodesign sign "$APP" >/dev/null 2>&1 || die "rcodesign sign failed"
    sig="$(rcodesign print-signature-info "$APP/Contents/MacOS/$BIN")"
    [[ "$(grep -c 'flags: CodeSignatureFlags(ADHOC)' <<<"$sig")" -ge 2 ]] || die "both slices must be ad-hoc signed"
    grep -q "identifier: $IDENT" <<<"$sig" || die "signature identifier is not $IDENT"
    [[ -f "$APP/Contents/_CodeSignature/CodeResources" ]] || die "bundle CodeResources missing"
    echo "    ad-hoc signed, identifier $IDENT, both slices"

    rm -f "$DIST/$NAME-macos-universal.zip"
    (cd "$STAGE" && zip -q -r -y -X "$DIST/$NAME-macos-universal.zip" "$NAME.app")
    file "$APP/Contents/MacOS/$BIN"
    echo "dist/$NAME-macos-universal.zip ($(size "$DIST/$NAME-macos-universal.zip"))"
fi
