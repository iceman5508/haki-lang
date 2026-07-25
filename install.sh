#!/bin/sh
# Haki language installer — https://haki-lang.dev
# Usage: curl -fsSL https://haki-lang.dev/install.sh | sh

set -e

REPO="haki-lang/haki"
VERSION="${HAKI_VERSION:-latest}"
INSTALL_DIR="${HAKI_INSTALL:-$HOME/.local/bin}"

# Detect platform
OS=$(uname -s | tr '[:upper:]' '[:lower:]')
ARCH=$(uname -m)
case "$ARCH" in
    x86_64|amd64)  ARCH="x86_64" ;;
    arm64|aarch64) ARCH="arm64" ;;
    *)
        echo "Unsupported architecture: $ARCH"
        exit 1
        ;;
esac

case "$OS" in
    linux)  PLATFORM="linux-$ARCH" ;;
    darwin) PLATFORM="macos-$ARCH" ;;
    *)
        echo "Unsupported OS: $OS"
        echo "For Windows, use: winget install haki-lang.haki"
        exit 1
        ;;
esac

# Resolve latest version
if [ "$VERSION" = "latest" ]; then
    VERSION=$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" | \
              grep '"tag_name"' | sed 's/.*"tag_name": "\(.*\)".*/\1/')
fi

ARCHIVE="hakic-${PLATFORM}.tar.gz"
URL="https://github.com/$REPO/releases/download/$VERSION/$ARCHIVE"

echo "Installing Haki $VERSION for $PLATFORM..."
echo "Downloading from: $URL"

TMP=$(mktemp -d)
trap "rm -rf $TMP" EXIT

curl -fsSL "$URL" -o "$TMP/$ARCHIVE"
tar -xzf "$TMP/$ARCHIVE" -C "$TMP"

mkdir -p "$INSTALL_DIR"
cp "$TMP/hakic" "$INSTALL_DIR/"
chmod +x "$INSTALL_DIR/hakic"

# Optional: symlink as 'haki'
ln -sf "$INSTALL_DIR/hakic" "$INSTALL_DIR/haki" 2>/dev/null || true

echo ""
echo "Haki $VERSION installed to $INSTALL_DIR/hakic"
echo ""
# Check if on PATH
if ! echo "$PATH" | grep -q "$INSTALL_DIR"; then
    echo "Add to your shell config:"
    echo "  export PATH=\"$INSTALL_DIR:\$PATH\""
    echo ""
fi
echo "Test: hakic run hello.haki"
