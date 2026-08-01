#!/usr/bin/env bash
set -euo pipefail

version="${PINORA_VERSION:-$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -n1)}"
platform="${PINORA_PLATFORM:-$(uname -s | tr '[:upper:]' '[:lower:]')}"
arch="${PINORA_ARCH:-$(uname -m)}"
case "$platform" in
  linux) platform_name="linux"; arch_name="x86_64" ;;
  darwin) platform_name="macos"; arch_name="${arch/arm64/aarch64}" ;;
  *) echo "unsupported unix platform: $platform" >&2; exit 2 ;;
esac

out="${PINORA_OUTPUT_DIR:-target/package}"
rm -rf "$out" target/package-stage
mkdir -p "$out" target/package-stage
cargo build --release --locked
binary="target/release/pinora"
test -x "$binary"

if [[ "$platform_name" == linux ]]; then
  deb_version="$(printf '%s' "$version" | sed 's/-/~/g')"
  stage="target/package-stage/linux"
  install -Dm755 "$binary" "$stage/usr/bin/pinora"
  install -Dm644 packaging/pinora.desktop "$stage/usr/share/applications/pinora.desktop"
  tarball="$out/pinora-${version}-${platform_name}-${arch_name}.tar.gz"
  tar -C "$stage" -czf "$tarball" .

  if command -v dpkg-deb >/dev/null 2>&1; then
    deb_root="target/package-stage/deb"
    mkdir -p "$deb_root/DEBIAN"
    install -Dm755 "$binary" "$deb_root/usr/bin/pinora"
    install -Dm644 packaging/pinora.desktop "$deb_root/usr/share/applications/pinora.desktop"
    cat > "$deb_root/DEBIAN/control" <<EOF
Package: pinora
Version: ${deb_version}
Section: graphics
Priority: optional
Architecture: amd64
Maintainer: Neo <yhyzgn@gmail.com>
Description: Cross-platform screenshot, annotation and pin workbench
EOF
    dpkg-deb --build "$deb_root" "$out/pinora_${version//-/_}_${arch_name}.deb" >/dev/null
  fi

  if command -v rpmbuild >/dev/null 2>&1; then
    rpm_release="$(printf '%s' "$version" | sed -n 's/^[^-]*-//; s/[^A-Za-z0-9.]/./g; p')"
    if [[ -z "$rpm_release" ]]; then rpm_release="1"; else rpm_release="1.${rpm_release}"; fi
    rpm_root="$(pwd)/target/package-stage/rpm"
    mkdir -p "$rpm_root"/{BUILD,RPMS,SOURCES,SPECS,SRPMS}
    cp "$binary" "$rpm_root/SOURCES/pinora"
    cat > "$rpm_root/SPECS/pinora.spec" <<EOF
Name: pinora
Version: ${version%%-*}
Release: ${rpm_release}
Summary: Cross-platform screenshot, annotation and pin workbench
License: MIT
BuildArch: x86_64

%description
Pinora screenshot and annotation workbench.

%install
mkdir -p %{buildroot}/usr/bin
install -m 0755 %{_sourcedir}/pinora %{buildroot}/usr/bin/pinora

%files
/usr/bin/pinora
EOF
    rpmbuild --define "_topdir $rpm_root" --define "_version ${version%%-*}" -bb "$rpm_root/SPECS/pinora.spec" >/dev/null
    find "$rpm_root/RPMS" -type f -name '*.rpm' -exec cp {} "$out/" \;
  fi
else
  app="target/package-stage/Pinora.app"
  mkdir -p "$app/Contents/MacOS"
  install -m 755 "$binary" "$app/Contents/MacOS/pinora"
  install -d "$app/Contents/Resources"
  cat > "$app/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleDisplayName</key><string>Pinora</string>
<key>CFBundleExecutable</key><string>pinora</string>
<key>CFBundleIdentifier</key><string>com.pinora.desktop</string>
<key>CFBundleName</key><string>Pinora</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleShortVersionString</key><string>${version}</string>
<key>CFBundleVersion</key><string>${version//[^0-9.]/}</string>
</dict></plist>
EOF
  ditto -c -k --sequesterRsrc --keepParent "$app" "$out/pinora-${version}-${platform_name}-${arch_name}.zip"
  hdiutil create -volname Pinora -srcfolder "$app" -ov -format UDZO "$out/pinora-${version}-${platform_name}-${arch_name}.dmg" >/dev/null
fi

(
  cd "$out"
  find . -maxdepth 1 -type f ! -name SHA256SUMS.txt -print0 | sort -z | xargs -0 shasum -a 256 > SHA256SUMS.txt
)
printf 'Packaged Pinora %s for %s/%s in %s\n' "$version" "$platform_name" "$arch_name" "$out"
