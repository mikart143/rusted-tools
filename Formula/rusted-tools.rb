class RustedTools < Formula
  desc "High-performance MCP proxy server for unified access to multiple Model Context Protocol servers"
  homepage "https://github.com/mikart143/rusted-tools"
  license "MIT"
  version "VERSION_PLACEHOLDER"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/mikart143/rusted-tools/releases/download/vVERSION_PLACEHOLDER/rusted-tools-VERSION_PLACEHOLDER-aarch64-apple-darwin.tar.gz"
      sha256 "SHA256_MACOS_ARM64_PLACEHOLDER"
    else
      url "https://github.com/mikart143/rusted-tools/releases/download/vVERSION_PLACEHOLDER/rusted-tools-VERSION_PLACEHOLDER-x86_64-apple-darwin.tar.gz"
      sha256 "SHA256_MACOS_X86_64_PLACEHOLDER"
    end
  end

  on_linux do
    if Hardware::CPU.arm?
      url "https://github.com/mikart143/rusted-tools/releases/download/vVERSION_PLACEHOLDER/rusted-tools-VERSION_PLACEHOLDER-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "SHA256_LINUX_ARM64_PLACEHOLDER"
    else
      url "https://github.com/mikart143/rusted-tools/releases/download/vVERSION_PLACEHOLDER/rusted-tools-VERSION_PLACEHOLDER-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "SHA256_LINUX_X86_64_PLACEHOLDER"
    end
  end

  def install
    bin.install "rusted-tools"
    etc.install "config.toml.example" => "rusted-tools/config.toml.example"
  end

  def caveats
    <<~EOS
      An example configuration file has been installed to:
        #{etc}/rusted-tools/config.toml.example

      To get started, copy it to your preferred location:
        cp #{etc}/rusted-tools/config.toml.example ~/.config/rusted-tools/config.toml

      Then run:
        rusted-tools --config ~/.config/rusted-tools/config.toml
    EOS
  end

  test do
    assert_match "rusted-tools", shell_output("#{bin}/rusted-tools --help")
  end
end
