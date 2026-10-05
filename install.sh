#!/bin/bash
# Build Caprice and install it for the current user: the program, its icon and the launcher entry.
set -euo pipefail
cd "$(dirname "$0")"

cargo build --release

if [[ "$(uname)" == "Darwin" ]]; then
  app="$HOME/Applications/Caprice.app"
  rm -rf "$app"
  mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"

  install -m755 target/release/caprice "$app/Contents/MacOS/caprice"

  # Build the .icns from the pre-rendered PNG; sips and iconutil ship with macOS, no extra tools needed.
  tmpdir=$(mktemp -d)
  iconset="$tmpdir/Caprice.iconset"
  mkdir -p "$iconset"
  for size in 16 32 128 256 512; do
    sips -z "$size" "$size" assets/icon-256.png --out "$iconset/icon_${size}x${size}.png" >/dev/null
    sips -z "$((size * 2))" "$((size * 2))" assets/icon-256.png --out "$iconset/icon_${size}x${size}@2x.png" >/dev/null
  done
  iconutil -c icns "$iconset" -o "$app/Contents/Resources/icon.icns"
  rm -rf "$tmpdir"

  cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>Caprice</string>
  <key>CFBundleDisplayName</key><string>Caprice</string>
  <key>CFBundleExecutable</key><string>caprice</string>
  <key>CFBundleIconFile</key><string>icon.icns</string>
  <key>CFBundleIdentifier</key><string>com.caprice.app</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>0.1.0</string>
  <key>CFBundleVersion</key><string>0.1.0</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>CFBundleDocumentTypes</key>
  <array>
    <dict>
      <key>CFBundleTypeName</key><string>Caprice Story</string>
      <key>CFBundleTypeExtensions</key><array><string>caprice</string></array>
      <key>CFBundleTypeRole</key><string>Editor</string>
      <key>CFBundleTypeIconFile</key><string>icon.icns</string>
    </dict>
  </array>
</dict>
</plist>
PLIST

  echo "Installed Caprice to $app"
else
  bin="$HOME/.local/bin"
  apps="$HOME/.local/share/applications"
  icons="$HOME/.local/share/icons/hicolor"
  mkdir -p "$bin" "$apps" "$icons/scalable/apps"

  install -m755 target/release/caprice "$bin/caprice"

  install -m644 assets/icon.svg "$icons/scalable/apps/caprice.svg"
  # Fixed-size PNGs, for the few panels that cannot show the SVG; skipped without librsvg.
  if command -v rsvg-convert >/dev/null; then
    for size in 16 24 32 48 64 128 256 512; do
      mkdir -p "$icons/${size}x${size}/apps"
      rsvg-convert -w "$size" -h "$size" assets/icon.svg -o "$icons/${size}x${size}/apps/caprice.png"
    done
  else
    echo "rsvg-convert not found: installing the SVG icon only"
  fi

  cat > "$apps/caprice.desktop" <<DESKTOP
[Desktop Entry]
Type=Application
Name=Caprice
GenericName=Word Processor
Comment=A word processor for creative story telling
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
fi
