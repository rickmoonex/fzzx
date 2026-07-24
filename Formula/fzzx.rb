class Fzzx < Formula
  desc "Small, scriptable fuzzy picker for macOS"
  homepage "https://github.com/rickmoonex/fzzx"
  version "0.1.0"
  license "MIT"

  depends_on :macos

  if Hardware::CPU.arm?
    url "https://github.com/rickmoonex/fzzx/releases/download/v0.1.0/fzzx-v0.1.0-aarch64-apple-darwin.tar.gz"
    sha256 "0000000000000000000000000000000000000000000000000000000000000000"
  else
    url "https://github.com/rickmoonex/fzzx/releases/download/v0.1.0/fzzx-v0.1.0-x86_64-apple-darwin.tar.gz"
    sha256 "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"
  end

  def install
    bin.install "fzzx"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/fzzx --version")
  end
end
