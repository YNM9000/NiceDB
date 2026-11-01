#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ICONS="$ROOT/app/src-tauri/icons"
BIN_DIR="${XDG_BIN_HOME:-$HOME/.local/bin}"
DATA="${XDG_DATA_HOME:-$HOME/.local/share}"
APPS="$DATA/applications"
HICOLOR="$DATA/icons/hicolor"
NAME=nicedb

die() { printf '\033[31merror:\033[0m %s\n' "$*" >&2; exit 1; }

refresh() {
    if command -v update-desktop-database >/dev/null 2>&1; then update-desktop-database "$APPS" || true; fi
    if command -v gtk-update-icon-cache >/dev/null 2>&1; then gtk-update-icon-cache -f -t -q "$HICOLOR" || true; fi
}

icons=(32x32:32x32.png 128x128:128x128.png 256x256:128x128@2x.png 512x512:icon.png)

if [[ "${1:-}" == "--uninstall" ]]; then
    rm -f "$BIN_DIR/$NAME" "$APPS/$NAME.desktop" "$HICOLOR/scalable/apps/$NAME.svg"
    for i in "${icons[@]}"; do rm -f "$HICOLOR/${i%%:*}/apps/$NAME.png"; done
    refresh
    echo "removed $NAME"
    exit 0
fi

SRC="${1:-$ROOT/dist/NiceDB-linux-x86_64}"
[[ -f "$SRC" ]] || die "no binary at $SRC (run scripts/build-all.sh linux first)"
file -b "$SRC" | grep -q 'ELF 64-bit.*x86-64' || die "$SRC is not an x86_64 Linux binary"

mkdir -p "$BIN_DIR" "$APPS"
install -m 755 "$SRC" "$BIN_DIR/.$NAME.new"
mv -f "$BIN_DIR/.$NAME.new" "$BIN_DIR/$NAME"

for i in "${icons[@]}"; do
    install -D -m 644 "$ICONS/${i#*:}" "$HICOLOR/${i%%:*}/apps/$NAME.png"
done
install -D -m 644 "$ICONS/icon.svg" "$HICOLOR/scalable/apps/$NAME.svg"

cat > "$APPS/$NAME.desktop" <<DESKTOP
[Desktop Entry]
Type=Application
Name=NiceDB
GenericName=Database client
GenericName[it]=Client per database
Comment=Explore, query and edit your databases
Comment[it]=Esplora, interroga e modifica i tuoi database
Exec=$BIN_DIR/$NAME
Icon=$NAME
Terminal=false
Categories=Development;Database;
Keywords=database;sql;postgres;mysql;sqlite;duckdb;
StartupWMClass=$NAME
DESKTOP
chmod 644 "$APPS/$NAME.desktop"

refresh
echo "installed $BIN_DIR/$NAME ($(du -h "$BIN_DIR/$NAME" | cut -f1)), $APPS/$NAME.desktop, icons in $HICOLOR"
