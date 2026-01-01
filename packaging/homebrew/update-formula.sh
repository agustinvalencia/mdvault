#!/bin/bash
# Script to update the Homebrew formula after a release
# Usage: ./update-formula.sh 0.1.0

set -e

VERSION="${1:-}"
if [ -z "$VERSION" ]; then
    echo "Usage: $0 <version>"
    echo "Example: $0 0.1.0"
    exit 1
fi

REPO="agustinvalencia/mdvault"
BASE_URL="https://github.com/${REPO}/releases/download/v${VERSION}"

echo "Fetching SHA256 checksums for v${VERSION}..."

# Download and extract SHA256 values
MACOS_INTEL_SHA=$(curl -sL "${BASE_URL}/mdv-${VERSION}-x86_64-apple-darwin.tar.gz.sha256" | awk '{print $1}')
MACOS_ARM_SHA=$(curl -sL "${BASE_URL}/mdv-${VERSION}-aarch64-apple-darwin.tar.gz.sha256" | awk '{print $1}')
LINUX_INTEL_SHA=$(curl -sL "${BASE_URL}/mdv-${VERSION}-x86_64-unknown-linux-gnu.tar.gz.sha256" | awk '{print $1}')
LINUX_ARM_SHA=$(curl -sL "${BASE_URL}/mdv-${VERSION}-aarch64-unknown-linux-gnu.tar.gz.sha256" | awk '{print $1}')

echo "SHA256 checksums:"
echo "  macOS Intel: ${MACOS_INTEL_SHA}"
echo "  macOS ARM:   ${MACOS_ARM_SHA}"
echo "  Linux Intel: ${LINUX_INTEL_SHA}"
echo "  Linux ARM:   ${LINUX_ARM_SHA}"

# Generate updated formula
cat > mdvault.rb << EOF
class Mdvault < Formula
  desc "CLI tool for managing markdown vaults with structured notes and validation"
  homepage "https://github.com/agustinvalencia/mdvault"
  version "${VERSION}"
  license "MIT"

  on_macos do
    on_intel do
      url "https://github.com/agustinvalencia/mdvault/releases/download/v${VERSION}/mdv-${VERSION}-x86_64-apple-darwin.tar.gz"
      sha256 "${MACOS_INTEL_SHA}"
    end
    on_arm do
      url "https://github.com/agustinvalencia/mdvault/releases/download/v${VERSION}/mdv-${VERSION}-aarch64-apple-darwin.tar.gz"
      sha256 "${MACOS_ARM_SHA}"
    end
  end

  on_linux do
    on_intel do
      url "https://github.com/agustinvalencia/mdvault/releases/download/v${VERSION}/mdv-${VERSION}-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "${LINUX_INTEL_SHA}"
    end
    on_arm do
      url "https://github.com/agustinvalencia/mdvault/releases/download/v${VERSION}/mdv-${VERSION}-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "${LINUX_ARM_SHA}"
    end
  end

  def install
    bin.install "mdv"
  end

  test do
    system "#{bin}/mdv", "--version"
  end
end
EOF

echo ""
echo "Formula updated! Copy mdvault.rb to your homebrew-tap repository."
