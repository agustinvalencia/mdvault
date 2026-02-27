class Mdvault < Formula
  desc "CLI tool for managing markdown vaults with structured notes and validation"
  homepage "https://github.com/agustinvalencia/mdvault"
  version "0.4.2"
  license "MIT"

  on_macos do
    on_intel do
      url "https://github.com/agustinvalencia/mdvault/releases/download/v0.4.2/mdv-0.4.2-x86_64-apple-darwin.tar.gz"
      sha256 "1391435e1ff3611345f67db4b49e21cd8515919f9b6f712a7a9d72ab734c6022"
    end
    on_arm do
      url "https://github.com/agustinvalencia/mdvault/releases/download/v0.4.2/mdv-0.4.2-aarch64-apple-darwin.tar.gz"
      sha256 "8e175b141c40572f0f3be33780e4e39ef73e3d5e222a5ebb63277b6dbc060854"
    end
  end

  on_linux do
    on_intel do
      url "https://github.com/agustinvalencia/mdvault/releases/download/v0.4.2/mdv-0.4.2-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "5b1cef1f1610d172b058a1c3e9a92dce6f822ebf9304696e384be4137d84876e"
    end
    on_arm do
      url "https://github.com/agustinvalencia/mdvault/releases/download/v0.4.2/mdv-0.4.2-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "9707aa5caa779e6e9206bbcf7334db04870e1d4accc3ecdc38309d397e7585dc"
    end
  end

  def install
    bin.install "mdv"
  end

  test do
    system "#{bin}/mdv", "--version"
  end
end
