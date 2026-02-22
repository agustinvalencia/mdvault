class Mdvault < Formula
  desc "CLI tool for managing markdown vaults with structured notes and validation"
  homepage "https://github.com/agustinvalencia/mdvault"
  version "0.3.6"
  license "MIT"

  on_macos do
    on_intel do
      url "https://github.com/agustinvalencia/mdvault/releases/download/v0.3.6/mdv-0.3.6-x86_64-apple-darwin.tar.gz"
      sha256 "46070cf83af742df1a1416f770a4575b8ff42c900cf6c5847b20d53cbd9b6d03"
    end
    on_arm do
      url "https://github.com/agustinvalencia/mdvault/releases/download/v0.3.6/mdv-0.3.6-aarch64-apple-darwin.tar.gz"
      sha256 "458eeec771733383cafc4126e1c34f267d6771eae03aa4153638754bd13b1059"
    end
  end

  on_linux do
    on_intel do
      url "https://github.com/agustinvalencia/mdvault/releases/download/v0.3.6/mdv-0.3.6-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "22bede0db41037ce944905b89cec4e3f85b4534bded8993e3c9b484b893b9604"
    end
    on_arm do
      url "https://github.com/agustinvalencia/mdvault/releases/download/v0.3.6/mdv-0.3.6-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "e671a446ea1827956898562abc1bf736dbdad0dc0312f60190dd6c4d896ab8c0"
    end
  end

  def install
    bin.install "mdv"
  end

  test do
    system "#{bin}/mdv", "--version"
  end
end
