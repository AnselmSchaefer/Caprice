#!/bin/bash
# Build Caprice and install it for the current user: the program, its icon and the launcher entry.
set -euo pipefail
cd "$(dirname "$0")"

cargo build --release

bin="$HOME/.local/bin"
apps="$HOME/.local/share/applications"
icons="$HOME/.local/share/icons/hicolor"
mkdir -p "$bin" "$apps" "$icons/scalable/apps"

install -m755 target/release/caprice "$bin/caprice"

install -m644 assets/icon.svg "$icons/scalable/apps/caprice.svg"
for size in 16 24 32 48 64 128 256 512; do
  mkdir -p "$icons/${size}x${size}/apps"
  rsvg-convert -w "$size" -h "$size" assets/icon.svg -o "$icons/${size}x${size}/apps/caprice.png"
done

cat > "$apps/caprice.desktop" <<DESKTOP
[Desktop Entry]
Type=Application
Name=Caprice
GenericName=Word Processor
Comment=A minimal word processor with page flipping
Exec=$bin/caprice %f
Icon=caprice
Terminal=false
Categories=Office;WordProcessor;TextEditor;
Keywords=word;document;write;text;
StartupWMClass=Caprice
DESKTOP

update-desktop-database "$apps" 2>/dev/null || true
gtk-update-icon-cache -f -t "$icons" 2>/dev/null || true
echo "Installed Caprice to $bin/caprice"
