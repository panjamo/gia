#!/bin/bash

# GIA macOS Package Builder
# Creates a user-installable .pkg for GIA CLI tool

set -e

# Configuration
PACKAGE_NAME="gia"
PACKAGE_VERSION="0.1.182"
PACKAGE_IDENTIFIER="com.gia.cli"
BUILD_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(dirname "$BUILD_DIR")"
PAYLOAD_DIR="$BUILD_DIR/payload"
SCRIPTS_DIR="$BUILD_DIR/scripts"
OUTPUT_DIR="$ROOT_DIR"
COMPONENT_PKG="$BUILD_DIR/gia-component.pkg"
DISTRIBUTION_XML="$BUILD_DIR/Distribution.xml"
FINAL_PKG="$OUTPUT_DIR/${PACKAGE_NAME}-${PACKAGE_VERSION}-installer.pkg"

echo "=========================================="
echo "GIA Package Builder"
echo "=========================================="
echo "Package: $PACKAGE_NAME"
echo "Version: $PACKAGE_VERSION"
echo "Identifier: $PACKAGE_IDENTIFIER"
echo ""˚

# Step 1: Copy binaries to payload
echo "[1/6] Preparing payload..."
echo "  Copying gia to payload/bin/"
cp "$ROOT_DIR/bin/gia" "$PAYLOAD_DIR/bin/gia"
echo "  Copying giagui to payload/bin/"
cp "$ROOT_DIR/bin/giagui" "$PAYLOAD_DIR/bin/giagui"

# Step 2: Create component package
echo ""
echo "[2/6] Building component package..."
pkgbuild \
    --root "$PAYLOAD_DIR" \
    --scripts "$SCRIPTS_DIR" \
    --identifier "$PACKAGE_IDENTIFIER" \
    --version "$PACKAGE_VERSION" \
    --install-location "/tmp/gia-install" \
    "$COMPONENT_PKG"

if [ ! -f "$COMPONENT_PKG" ]; then
    echo "Error: Failed to create component package"
    exit 1
fi
echo "  Component package created: $COMPONENT_PKG"

# Step 3: Synthesize Distribution.xml
echo ""
echo "[3/6] Generating Distribution.xml..."
productbuild --synthesize \
    --package "$COMPONENT_PKG" \
    "$DISTRIBUTION_XML"

if [ ! -f "$DISTRIBUTION_XML" ]; then
    echo "Error: Failed to generate Distribution.xml"
    exit 1
fi
echo "  Distribution.xml generated"

# Step 4: Modify Distribution.xml to require admin (installs to current user via postinstall)
echo ""
echo "[4/6] Modifying Distribution.xml to require admin authentication..."

# Create a modified Distribution.xml
# Using enable_localSystem=true to force admin auth, but postinstall installs to current user only
cat > "${DISTRIBUTION_XML}.tmp" << 'EOF'
<?xml version="1.0" encoding="utf-8" standalone="no"?>
<installer-gui-script minSpecVersion="1">
    <title>GIA - AI CLI Tool</title>
    <organization>com.gia</organization>
    <domains enable_anywhere="false" enable_currentUserHome="false" enable_localSystem="true"/>
    <options customize="never" require-scripts="true" hostArchitectures="arm64,x86_64"/>
    <welcome file="welcome.html" mime-type="text/html"/>
    <conclusion file="conclusion.html" mime-type="text/html"/>
    <choices-outline>
        <line choice="default">
            <line choice="com.gia.cli"/>
        </line>
    </choices-outline>
    <choice id="default"/>
    <choice id="com.gia.cli" visible="false">
        <pkg-ref id="com.gia.cli"/>
    </choice>
    <pkg-ref id="com.gia.cli" version="0.1.182" onConclusion="none">gia-component.pkg</pkg-ref>
</installer-gui-script>
EOF

mv "${DISTRIBUTION_XML}.tmp" "$DISTRIBUTION_XML"
echo "  Distribution.xml modified to require admin authentication"

# Step 5: Create welcome and conclusion HTML files
echo ""
echo "[5/6] Creating welcome and conclusion pages..."

cat > "$BUILD_DIR/welcome.html" << 'EOF'
<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <style>
        body {
            font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Helvetica, Arial, sans-serif;
            padding: 20px;
            color: #333;
        }
        h1 {
            color: #1d1d1f;
            font-size: 24px;
        }
        p {
            line-height: 1.6;
            font-size: 14px;
        }
        .notice {
            background: #fffbea;
            border: 2px solid #f59e0b;
            border-radius: 6px;
            padding: 16px;
            margin: 20px 0;
        }
        .notice-title {
            font-weight: bold;
            font-size: 15px;
            color: #92400e;
            margin: 0 0 8px 0;
        }
        .notice-text {
            font-size: 14px;
            color: #78350f;
            margin: 0;
        }
        ul {
            margin: 10px 0;
        }
        li {
            margin: 6px 0;
            font-size: 14px;
        }
        strong {
            font-weight: 600;
        }
    </style>
</head>
<body>
    <h1>Welcome to GIA Installer</h1>
    <p>This installer will install the GIA AI CLI tool on your system.</p>

    <div class="notice">
        <p class="notice-title">⚠️  Administrator Password Required</p>
        <p class="notice-text">This installer requires an administrator password. Files will be installed to the current user's home directory only (~/bin).</p>
    </div>

    <p><strong>What will be installed:</strong></p>
    <ul>
        <li><strong>gia</strong> - CLI tool installed to ~/bin/gia</li>
        <li><strong>giagui</strong> - GUI helper installed to ~/bin/giagui</li>
    </ul>
    <p>The installer will create ~/bin if it doesn't exist and configure your shell to include it in your PATH.</p>
</body>
</html>
EOF

cat > "$BUILD_DIR/conclusion.html" << 'EOF'
<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <style>
        body {
            font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Helvetica, Arial, sans-serif;
            padding: 20px;
            color: #333;
        }
        h1 {
            color: #2e7d32;
            font-size: 24px;
        }
        p {
            line-height: 1.6;
            font-size: 14px;
        }
        ul, ol {
            margin: 10px 0;
            padding-left: 24px;
        }
        li {
            margin: 6px 0;
            font-size: 14px;
        }
        code {
            background: #f5f5f5;
            padding: 3px 8px;
            border-radius: 4px;
            font-family: Monaco, "Courier New", monospace;
            font-size: 13px;
            color: #d73a49;
        }
        strong {
            font-weight: 600;
        }
    </style>
</head>
<body>
    <h1>✓ Installation Complete!</h1>
    <p>GIA has been successfully installed to your system.</p>

    <p><strong>Installed files:</strong></p>
    <ul>
        <li><strong>gia</strong>: ~/bin/gia</li>
        <li><strong>giagui</strong>: ~/bin/giagui</li>
    </ul>

    <p><strong>Next steps:</strong></p>
    <ol>
        <li>Open a new terminal window or run: <code>source ~/.zshrc</code></li>
        <li>Verify installation: <code>gia --version</code></li>
        <li>Get help: <code>gia --help</code></li>
    </ol>

    <p>You can now start using GIA by typing <code>gia "your prompt"</code> in the terminal.</p>
</body>
</html>
EOF

echo "  Welcome and conclusion pages created"

# Step 6: Build final product package
echo ""
echo "[6/6] Building final product package..."
productbuild \
    --distribution "$DISTRIBUTION_XML" \
    --package-path "$BUILD_DIR" \
    --resources "$BUILD_DIR" \
    "$FINAL_PKG"

if [ ! -f "$FINAL_PKG" ]; then
    echo "Error: Failed to create final package"
    exit 1
fi

# Get file size
FILESIZE=$(du -h "$FINAL_PKG" | cut -f1)

echo ""
echo "=========================================="
echo "Package built successfully!"
echo "=========================================="
echo "Output: $FINAL_PKG"
echo "Size: $FILESIZE"
echo ""
echo "To install (requires admin password, installs to current user only):"
echo "  open $FINAL_PKG"
echo ""
echo "Or from command line:"
echo "  sudo installer -pkg \"$FINAL_PKG\" -target /"
echo ""
echo "Cleaning up temporary files..."
rm -f "$COMPONENT_PKG"
rm -f "$DISTRIBUTION_XML"
rm -f "$BUILD_DIR/welcome.html"
rm -f "$BUILD_DIR/conclusion.html"

echo "Done!"
echo "=========================================="
