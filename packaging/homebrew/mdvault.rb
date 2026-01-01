# Homebrew formula for mdvault
# To use: copy this file to your homebrew-tap repository
# Users install via: brew install agustinvalencia/tap/mdvault

class Mdvault < Formula
  desc "CLI tool for managing markdown vaults with structured notes and validation"
  homepage "https://github.com/agustinvalencia/mdvault"
  version "VERSION_PLACEHOLDER"
  license "MIT"

  on_macos do
    on_intel do
      url "https://github.com/agustinvalencia/mdvault/releases/download/vVERSION_PLACEHOLDER/mdv-VERSION_PLACEHOLDER-x86_64-apple-darwin.tar.gz"
      sha256 "SHA256_MACOS_INTEL"
    end
    on_arm do
      url "https://github.com/agustinvalencia/mdvault/releases/download/vVERSION_PLACEHOLDER/mdv-VERSION_PLACEHOLDER-aarch64-apple-darwin.tar.gz"
      sha256 "SHA256_MACOS_ARM"
    end
  end

  on_linux do
    on_intel do
      url "https://github.com/agustinvalencia/mdvault/releases/download/vVERSION_PLACEHOLDER/mdv-VERSION_PLACEHOLDER-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "SHA256_LINUX_INTEL"
    end
    on_arm do
      url "https://github.com/agustinvalencia/mdvault/releases/download/vVERSION_PLACEHOLDER/mdv-VERSION_PLACEHOLDER-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "SHA256_LINUX_ARM"
    end
  end

  def install
    bin.install "mdv"
  end

  test do
    system "#{bin}/mdv", "--version"
  end
end
