class Fzzx < Formula
  desc "Small, scriptable fuzzy picker for macOS"
  homepage "https://github.com/rickmoonex/fzzx"
  version "0.1.1"
  license "MIT"

  depends_on :macos

  if Hardware::CPU.arm?
    url "https://github.com/rickmoonex/fzzx/releases/download/v0.1.1/fzzx-v0.1.1-aarch64-apple-darwin.tar.gz"
    sha256 "6204c23b442e390d07aebaa3e2df48449f48b0cf58f140c8a27dcd8ae99d8494"
  else
    url "https://github.com/rickmoonex/fzzx/releases/download/v0.1.1/fzzx-v0.1.1-x86_64-apple-darwin.tar.gz"
    sha256 "b9b201014f4dff831be71d910d92f712449bd17a5a29fc524ec2491a49504ac2"
  end

  def install
    bin.install "fzzx"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/fzzx --version")
  end
end
