class Mdvault < Formula
  desc "CLI tool for managing markdown vaults with structured notes and validation"
  homepage "https://github.com/agustinvalencia/mdvault"
  version "0.4.1"
  license "MIT"

  on_macos do
    on_intel do
      url "https://github.com/agustinvalencia/mdvault/releases/download/v0.4.1/mdv-0.4.1-x86_64-apple-darwin.tar.gz"
      sha256 "604ac9c0d2e7fe9b452cbce61dd94d0efc4160d7ae6ff3c584ac322761c3377d"
    end
    on_arm do
      url "https://github.com/agustinvalencia/mdvault/releases/download/v0.4.1/mdv-0.4.1-aarch64-apple-darwin.tar.gz"
      sha256 "78cdf8a774572e349b80c9ae88fa21cbd16876f7afae21b880ffb8b0f856ebb5"
    end
  end

  on_linux do
    on_intel do
      url "https://github.com/agustinvalencia/mdvault/releases/download/v0.4.1/mdv-0.4.1-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "381f3d461d50da489bcaf7425e18fcfbc2fa9a49f7f30a52b5ec0e1fcfdd0877"
    end
    on_arm do
      url "https://github.com/agustinvalencia/mdvault/releases/download/v0.4.1/mdv-0.4.1-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "0735f268433548076242efc1896b33259f5e12fa1c71d9605528ff6a2ed54ab2"
    end
  end

  def install
    bin.install "mdv"
  end

  test do
    system "#{bin}/mdv", "--version"
  end
end
