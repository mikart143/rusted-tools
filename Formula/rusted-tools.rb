class RustedTools < Formula
  desc "High-performance MCP proxy server for unified access to multiple Model Context Protocol servers"
  homepage "https://github.com/mikart143/rusted-tools"
  license "MIT"
  version "1.0.0"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/mikart143/rusted-tools/releases/download/v1.0.0/rusted-tools-1.0.0-aarch64-apple-darwin.tar.gz"
      sha256 "6c9d2a62fd87412fc98a0f3eb9c8ae32304e75d63d225d3a674308f8fe1fd3da"
    else
      url "https://github.com/mikart143/rusted-tools/releases/download/v1.0.0/rusted-tools-1.0.0-x86_64-apple-darwin.tar.gz"
      sha256 ""
    end
  end

  on_linux do
    if Hardware::CPU.arm?
      url "https://github.com/mikart143/rusted-tools/releases/download/v1.0.0/rusted-tools-1.0.0-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "875ec9a6695cc9a0f1b5257f016f211e38b356f01c6bf4deadde60d4909a2a07"
    else
      url "https://github.com/mikart143/rusted-tools/releases/download/v1.0.0/rusted-tools-1.0.0-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "3fd4d9937b30ef4bffdb68c5dc86f39e3630fdd1ac801a76f4d4f6a3d15a3bfd"
    end
  end

  def install
    bin.install "rusted-tools"
  end

  test do
    assert_match "rusted-tools", shell_output("#{bin}/rusted-tools --help")
  end
end
