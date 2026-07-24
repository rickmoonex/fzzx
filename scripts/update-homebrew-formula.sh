#!/bin/sh

set -eu

if [ "$#" -lt 3 ] || [ "$#" -gt 4 ]; then
  echo "usage: $0 VERSION ARM64_SHA256 X86_64_SHA256 [OUTPUT]" >&2
  exit 2
fi

version=$1
arm64_sha256=$2
x86_64_sha256=$3
output=${4:-Formula/fzzx.rb}

case "$version" in
  *[!0-9.]* | "") echo "invalid version: $version" >&2; exit 2 ;;
esac

for checksum in "$arm64_sha256" "$x86_64_sha256"; do
  case "$checksum" in
    *[!0-9a-f]* | "") echo "invalid SHA-256: $checksum" >&2; exit 2 ;;
  esac
  [ "${#checksum}" -eq 64 ] || {
    echo "invalid SHA-256 length: $checksum" >&2
    exit 2
  }
done

mkdir -p "$(dirname "$output")"
cat >"$output" <<EOF
class Fzzx < Formula
  desc "Small, scriptable fuzzy picker for macOS"
  homepage "https://github.com/rickmoonex/fzzx"
  version "$version"
  license "MIT"

  depends_on :macos

  if Hardware::CPU.arm?
    url "https://github.com/rickmoonex/fzzx/releases/download/v$version/fzzx-v$version-aarch64-apple-darwin.tar.gz"
    sha256 "$arm64_sha256"
  else
    url "https://github.com/rickmoonex/fzzx/releases/download/v$version/fzzx-v$version-x86_64-apple-darwin.tar.gz"
    sha256 "$x86_64_sha256"
  end

  def install
    bin.install "fzzx"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/fzzx --version")
  end
end
EOF
