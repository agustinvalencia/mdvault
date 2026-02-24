class Mdvault < Formula
  desc "CLI tool for managing markdown vaults with structured notes and validation"
  homepage "https://github.com/agustinvalencia/mdvault"
  version "0.4.0"
  license "MIT"

  on_macos do
    on_intel do
      url "https://github.com/agustinvalencia/mdvault/releases/download/v0.4.0/mdv-0.4.0-x86_64-apple-darwin.tar.gz"
      sha256 "9225ae869162782602fc2f41a42aea6fb45a891f75b8549cee2c7aeab66578d7"
    end
    on_arm do
      url "https://github.com/agustinvalencia/mdvault/releases/download/v0.4.0/mdv-0.4.0-aarch64-apple-darwin.tar.gz"
      sha256 "94691fc9e54faa73399c45cecbec1b9d6942d778ed3225960646730c4e96c1e4"
    end
  end

  on_linux do
    on_intel do
      url "https://github.com/agustinvalencia/mdvault/releases/download/v0.4.0/mdv-0.4.0-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "0f19554027c504062987bdb2401eca0c16292a073e766f403d362c7726c2e3ea"
    end
    on_arm do
      url "https://github.com/agustinvalencia/mdvault/releases/download/v0.4.0/mdv-0.4.0-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "1d5ee6775f5fb14ca155acad7a05019550e5e8e4a9aa3b656b6f930d3f2a66db"
    end
  end

  def install
    bin.install "mdv"
  end

  test do
    system "#{bin}/mdv", "--version"
  end
end
