# GIA macOS Package Builder

This directory contains scripts to build a user-installable macOS `.pkg` installer for GIA.

## Quick Start

```bash
# Build the installer package
./build-package.sh

# Output: ../gia-0.1.182-installer.pkg
```

## What Gets Built

The package builder creates a macOS installer that:

1. **Installs gia and giagui** to `~/bin/`
2. **Creates `~/bin` directory** if it doesn't exist
3. **Sets permissions** (makes binaries executable)
4. **Removes quarantine** attributes with `xattr -d com.apple.quarantine`
5. **Configures PATH** by adding `~/bin` to shell profiles

## Directory Structure

```
packaging/
├── payload/               # Temporary staging directory
│   ├── bin/              # gia binary staged here during build
│   └── Applications/     # giagui binary staged here during build
├── scripts/
│   └── postinstall       # Installation script (runs after package install)
├── build-package.sh      # Main build script
└── README.md             # This file
```

## Installation

After building, install the package (requires admin password, installs to current user only):

```bash
# GUI installation (recommended) - will prompt for admin password
open ../gia-0.1.182-installer.pkg

# Command-line installation
sudo installer -pkg ../gia-0.1.182-installer.pkg -target /
```

**Note:** Administrator password is required for installation, but the postinstall script detects the current user and installs files only to their ~/bin directory (not system-wide). Admin privileges are needed to remove quarantine attributes and configure shell profiles.

## What the Postinstall Script Does

The `postinstall` script (`scripts/postinstall`) handles the actual installation:

1. **Detects the current user** (not root, even though installer runs as root)
2. **Creates `~/bin`** if it doesn't exist
3. **Copies both binaries** (gia and giagui) from staging location to `~/bin/`
4. **Sets ownership** to the user
5. **Makes executable** with `chmod +x`
6. **Removes quarantine** with `xattr -d com.apple.quarantine`
7. **Updates shell profiles**:
   - `~/.zshrc` (default shell on modern macOS)
   - `~/.bash_profile` (bash users)
   - `~/.profile` (fallback)
8. **Prints installation summary** with next steps

## Build Process Details

The `build-package.sh` script follows these steps:

1. **Copies binaries** from `../bin/` to `payload/`
2. **Creates component package** using `pkgbuild`
   - Installs to staging location `/tmp/gia-install`
   - Includes postinstall script
3. **Generates Distribution.xml** using `productbuild --synthesize`
4. **Modifies Distribution.xml** to enable user home installation
   - Sets `enable_currentUserHome="true"`
   - Disables system-wide installation
5. **Creates HTML files** for welcome and conclusion screens
6. **Builds final package** using `productbuild`
7. **Cleans up** temporary files

## Customization

### Update Version

Edit `build-package.sh` and change:

```bash
PACKAGE_VERSION="0.1.182"
```

### Modify Installation Behavior

Edit `scripts/postinstall` to change:
- Installation paths
- Shell profile modifications
- Post-installation messages

### Change Package Appearance

Edit the HTML generation in `build-package.sh`:
- `welcome.html` - Welcome screen content
- `conclusion.html` - Success screen content

## Troubleshooting

### Package Not Building

**Check dependencies:**
```bash
which pkgbuild productbuild
# Should show paths to both tools (standard on macOS)
```

**Check file permissions:**
```bash
ls -l scripts/postinstall build-package.sh
# Both should be executable (rwxr-xr-x)
```

### Installation Issues

**Check logs:**
```bash
# View installation logs
tail -f /var/log/install.log
```

**Verify binaries exist:**
```bash
ls -l ~/bin/gia ~/bin/giagui
```

**Check PATH:**
```bash
echo $PATH | grep "$HOME/bin"
# Should show ~/bin in PATH after sourcing profile
```

**Manually source profile:**
```bash
source ~/.zshrc  # or ~/.bash_profile
gia --version
```

### Quarantine Not Removed

If you still get quarantine warnings:

```bash
# Manually remove quarantine
xattr -d com.apple.quarantine ~/bin/gia
xattr -d com.apple.quarantine ~/bin/giagui

# Verify
xattr -l ~/bin/gia  # Should show no quarantine attribute
```

## Package Signing

This package is currently **unsigned**. For distribution outside of development:

1. **Get Developer ID certificate** from Apple Developer Program
2. **Sign the package:**
   ```bash
   productsign --sign "Developer ID Installer: Your Name" \
       gia-0.1.182-installer.pkg \
       gia-0.1.182-installer-signed.pkg
   ```
3. **Verify signature:**
   ```bash
   pkgutil --check-signature gia-0.1.182-installer-signed.pkg
   ```

## Notes

- Package size: ~6.4 MB (includes both binaries)
- Target: macOS ARM64 and x86_64
- Requires: macOS 10.13+ (typical for modern packages)
- **Requires administrator password** for installation
- **User-specific installation** (postinstall detects current user and installs to their ~/bin only)
- Installs both binaries to `~/bin/` of the current user
- Safe to reinstall (idempotent)

## More Information

See the main [CLAUDE.md](../CLAUDE.md) for complete documentation about GIA and its usage.
