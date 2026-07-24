class Fzzx < Formula
  desc "Small, scriptable fuzzy picker for macOS"
  homepage "https://github.com/rickmoonex/fzzx"
  version "0.1.0"
  license "MIT"

  depends_on :macos

  if Hardware::CPU.arm?
    url "https://github.com/rickmoonex/fzzx/releases/download/v0.1.0/fzzx-v0.1.0-aarch64-apple-darwin.tar.gz"
    sha256 "2eba378e466ececb694ac1439cab54265bada86cc6ef13449c4f74718619f88d"
  else
    url "https://github.com/rickmoonex/fzzx/releases/download/v0.1.0/fzzx-v0.1.0-x86_64-apple-darwin.tar.gz"
    sha256 "d536f38ce41a453aa549a2fad81cb76732646c524b7cee9c1f65763301f6a6a0"
  end

  def install
    bin.install "fzzx"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/fzzx --version")
  end
end
