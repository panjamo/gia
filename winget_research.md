# Creating a Private WinGet Repository with GitHub Integration

## Critical clarification upfront

**GitHub Packages does not natively support WinGet repositories.** As of 2025, GitHub Packages only supports npm, RubyGems, Maven, Gradle, Docker, NuGet, and container registries—WinGet is not among them. However, you can absolutely use GitHub infrastructure to manage and distribute WinGet packages through alternative approaches.

This guide covers **practical alternatives using GitHub** for WinGet package distribution, including automated workflows and private repository hosting.

## Overview of viable approaches

You have three main options for WinGet package management with GitHub:

**Option 1: Automated publishing to public winget-pkgs repository** (recommended for open-source projects)
- Use GitHub Actions to automatically submit packages to Microsoft's community repository
- Simplest approach with broad user reach
- Packages become publicly available

**Option 2: Self-hosted private REST source with GitHub automation**
- Deploy a private WinGet REST API (Azure, self-hosted, or third-party)
- Use GitHub Actions to automatically publish to your private source
- Full control over package access

**Option 3: GitHub Pages with MSIX** (advanced, legacy approach)
- Host pre-indexed package database on GitHub Pages
- Complex setup requiring code signing
- Not recommended for new implementations

This guide focuses on **Options 1 and 2** as they represent the most practical modern approaches.

## Part 1: Automated Publishing to Public WinGet Repository

This approach uses GitHub Actions to automatically submit your packages to Microsoft's community winget-pkgs repository whenever you create a release.

### Repository structure for automated publishing

```
your-app-repo/
├── .github/
│   └── workflows/
│       └── winget-publish.yml
├── src/
│   └── (your application code)
├── installers/
│   └── (generated during build/release)
└── README.md
```

### Complete GitHub Actions workflow

Create `.github/workflows/winget-publish.yml`:

```yaml
name: Publish to WinGet

on:
  release:
    types: [published]  # Only triggers on published releases, not drafts
  workflow_dispatch:    # Allows manual triggering
    inputs:
      version:
        description: 'Package version to publish'
        required: true

jobs:
  publish-winget:
    name: Publish to WinGet Community Repository
    runs-on: windows-latest
    
    steps:
      - name: Get Release Information
        id: get_release_info
        run: |
          if ("${{ github.event_name }}" -eq "release") {
            # Automatic release trigger
            $version = "${{ github.event.release.tag_name }}"
            $github = Get-Content '${{ github.event_path }}' | ConvertFrom-Json
            $installerUrl = $github.release.assets | 
              Where-Object -Property name -match '\.msi$|\.exe$|\.msix$' | 
              Select-Object -ExpandProperty browser_download_url -First 1
          } else {
            # Manual workflow dispatch
            $version = "${{ github.event.inputs.version }}"
            $installerUrl = "https://github.com/${{ github.repository }}/releases/download/v${version}/YourApp-${version}.msi"
          }
          
          # Strip 'v' prefix if present
          $version = $version.TrimStart('v')
          
          echo "version=$version" >> $env:GITHUB_OUTPUT
          echo "installer_url=$installerUrl" >> $env:GITHUB_OUTPUT
        shell: powershell
      
      - name: Download WinGetCreate
        run: |
          Invoke-WebRequest -Uri https://aka.ms/wingetcreate/latest -OutFile wingetcreate.exe
        shell: powershell
      
      - name: Submit to WinGet
        run: |
          # Update manifest and submit PR to winget-pkgs
          .\wingetcreate.exe update Publisher.YourApp `
            --version ${{ steps.get_release_info.outputs.version }} `
            --urls ${{ steps.get_release_info.outputs.installer_url }} `
            --submit `
            --token ${{ secrets.WINGET_TOKEN }}
        shell: powershell
```

### Alternative: Using WinGet Releaser action

For a simpler setup, use the community-maintained action:

```yaml
name: Publish to WinGet (Simple)

on:
  release:
    types: [published]

jobs:
  publish:
    runs-on: windows-latest
    steps:
      - uses: vedantmgoyal9/winget-releaser@main
        with:
          identifier: Publisher.YourApp
          token: ${{ secrets.WINGET_TOKEN }}
          # Optional: filter specific installer types
          installers-regex: '\.msi$|\.exe$'
          # Optional: limit versions kept in repository
          max-versions-to-keep: 5
```

### Setting up GitHub token

1. **Create a GitHub Personal Access Token (PAT)**:
   - Go to GitHub Settings → Developer settings → Personal access tokens → Tokens (classic)
   - Click "Generate new token (classic)"
   - Select scope: **public_repo** (required)
   - Copy the token immediately

2. **Add token to repository secrets**:
   - Go to your repository → Settings → Secrets and variables → Actions
   - Click "New repository secret"
   - Name: `WINGET_TOKEN`
   - Value: Paste your PAT
   - Click "Add secret"

3. **Fork winget-pkgs repository**:
   - Go to https://github.com/microsoft/winget-pkgs
   - Click "Fork" to create a fork in your account
   - Install Pull App to keep fork synchronized: https://github.com/apps/pull

### Initial manual submission

Before automation works, you must manually submit your package once:

```bash
# Install wingetcreate
winget install wingetcreate

# Create new manifest
wingetcreate.exe new https://github.com/yourorg/yourapp/releases/download/v1.0.0/YourApp-1.0.0.msi

# Follow interactive prompts to fill in:
# - Publisher name
# - Package name
# - Description
# - License
# - Tags
# etc.

# Submit to winget-pkgs
wingetcreate.exe submit --token YOUR_GITHUB_TOKEN
```

After the initial submission is merged, future versions can be automated.

## Part 2: Private WinGet REST Source with GitHub Integration

For enterprise scenarios requiring private package distribution, you'll need a private WinGet REST source.

### Architecture overview

```
┌─────────────────────┐
│   GitHub Actions    │ ← Automatically publishes packages
│   (Your Repo)       │
└─────────┬───────────┘
          │ HTTPS/REST API
┌─────────▼───────────┐
│  Private WinGet     │
│  REST Source        │ ← Options: Azure, self-hosted, third-party
│  (API Server)       │
└─────────┬───────────┘
          │
┌─────────▼───────────┐
│  Package Storage    │
│  (Database/Files)   │
└─────────────────────┘
          │
┌─────────▼───────────┐
│  WinGet Clients     │ ← Enterprise users
└─────────────────────┘
```

### Option 2A: Microsoft official Azure-based solution

**Prerequisites:**
- Azure subscription
- PowerShell 7+

**Deployment steps:**

```powershell
# 1. Install Microsoft's WinGet REST Source module
Install-Module -Name Microsoft.WinGet.RestSource

# 2. Deploy to Azure
New-WinGetSource `
  -Name "CompanyPrivateRepo" `
  -ResourceGroup "winget-private-rg" `
  -SubscriptionId "your-subscription-id" `
  -Region "eastus" `
  -ImplementationPerformance "Basic" `
  -ShowConnectionInstructions

# Output will provide:
# - REST API URL
# - Function host key for management
# - Client connection instructions
```

**Cost considerations:**
- **Developer tier**: Uses free tiers where possible (~$0-10/month)
- **Basic tier**: Default production option (~$50-100/month)
- **Enhanced tier**: High-performance with API Management (~$200+/month)

### Option 2B: Self-hosted solutions (no Azure required)

#### Using WinGetty (Python-based)

**Deployment with Docker:**

```bash
# Clone repository
git clone https://github.com/thilojaeggi/WinGetty
cd WinGetty

# Configure environment
cat > .env << EOF
SECRET_KEY=your-secret-key-here
DATABASE_URL=sqlite:///wingetty.db
WINGETTY_ADMIN_PASSWORD=secure-password
EOF

# Deploy with Docker Compose
docker-compose up -d

# Access admin interface at http://localhost:8080
```

**Features:**
- Web-based package management interface
- SQLite or PostgreSQL backend
- OIDC authentication support
- No cloud dependencies

#### Using winget.pro (Commercial/Self-hosted)

```bash
# Quick setup (2 commands)
docker pull omahaconsulting/winget.pro
docker run -d -p 8080:80 omahaconsulting/winget.pro
```

**Features:**
- Self-hosted or cloud-hosted options
- Microsoft Entra ID integration
- Per-package access control
- No Azure dependencies

### GitHub Actions workflow for private source

Create `.github/workflows/publish-private.yml`:

```yaml
name: Publish to Private WinGet Source

on:
  release:
    types: [published]

jobs:
  publish-private:
    name: Publish to Company Repository
    runs-on: windows-latest
    
    steps:
      - name: Checkout
        uses: actions/checkout@v4
      
      - name: Get Release Info
        id: release
        run: |
          $version = "${{ github.event.release.tag_name }}".TrimStart('v')
          $installerUrl = "${{ github.event.release.assets[0].browser_download_url }}"
          echo "version=$version" >> $env:GITHUB_OUTPUT
          echo "installer_url=$installerUrl" >> $env:GITHUB_OUTPUT
        shell: powershell
      
      - name: Create Manifest Files
        run: |
          # Create manifest directory structure
          $publisherId = "YourCompany"
          $packageId = "YourApp"
          $version = "${{ steps.release.outputs.version }}"
          $manifestPath = "manifests\$($publisherId)\$($packageId)\$version"
          
          New-Item -ItemType Directory -Path $manifestPath -Force
          
          # Version manifest
          @"
          PackageIdentifier: $publisherId.$packageId
          PackageVersion: $version
          DefaultLocale: en-US
          ManifestType: version
          ManifestVersion: 1.6.0
          "@ | Out-File "$manifestPath\$publisherId.$packageId.yaml" -Encoding UTF8
          
          # Installer manifest
          $installerSha256 = (Get-FileHash -Path "installer.msi" -Algorithm SHA256).Hash
          
          @"
          PackageIdentifier: $publisherId.$packageId
          PackageVersion: $version
          InstallerType: msi
          Installers:
            - Architecture: x64
              InstallerUrl: ${{ steps.release.outputs.installer_url }}
              InstallerSha256: $installerSha256
              Scope: machine
          ManifestType: installer
          ManifestVersion: 1.6.0
          "@ | Out-File "$manifestPath\$publisherId.$packageId.installer.yaml" -Encoding UTF8
          
          # Locale manifest
          @"
          PackageIdentifier: $publisherId.$packageId
          PackageVersion: $version
          PackageLocale: en-US
          Publisher: Your Company Name
          PackageName: Your Application
          License: MIT
          ShortDescription: Brief description of your application
          ManifestType: defaultLocale
          ManifestVersion: 1.6.0
          "@ | Out-File "$manifestPath\$publisherId.$packageId.locale.en-US.yaml" -Encoding UTF8
        shell: powershell
      
      - name: Publish to Private Source
        run: |
          # Convert manifest to JSON and publish via REST API
          $manifestPath = "manifests\YourCompany\YourApp\${{ steps.release.outputs.version }}"
          
          # Read and combine manifest files
          $versionContent = Get-Content "$manifestPath\*.yaml" -Raw
          
          # POST to private REST source
          $headers = @{
              "Content-Type" = "application/json"
              "x-functions-key" = "${{ secrets.PRIVATE_SOURCE_KEY }}"
          }
          
          $body = $versionContent | ConvertFrom-Yaml | ConvertTo-Json -Depth 10
          
          Invoke-RestMethod `
            -Uri "${{ secrets.PRIVATE_SOURCE_URL }}/api/packageManifests" `
            -Method Post `
            -Headers $headers `
            -Body $body
        shell: powershell
```

### Required repository secrets

Add these to your repository secrets:

- `PRIVATE_SOURCE_URL`: Your REST API endpoint (e.g., `https://packages.company.com`)
- `PRIVATE_SOURCE_KEY`: API key or host key for authentication

## Part 3: Client Configuration

### Adding private source to WinGet clients

**For public winget-pkgs packages:**
```bash
# No configuration needed - works automatically
winget install Publisher.YourApp
```

**For private REST sources:**

```bash
# Basic private source (no authentication)
winget source add --name CompanyRepo --arg https://packages.company.com/api/ --type Microsoft.Rest

# With Microsoft Entra ID authentication
winget source add --name CompanyRepo --arg https://packages.company.com/api/ --type Microsoft.Rest --authorization EntraID

# Verify source was added
winget source list

# Install from private source
winget install YourCompany.YourApp --source CompanyRepo
```

### Enterprise deployment script

Create `Add-CompanyWinGetSource.ps1` for mass deployment:

```powershell
<#
.SYNOPSIS
    Adds company WinGet source to client machines
.DESCRIPTION
    Configures WinGet to use company private repository
    Requires: Administrator privileges
#>

param(
    [string]$SourceName = "CompanyRepo",
    [string]$SourceUrl = "https://packages.company.com/api/",
    [string]$AuthType = "None"  # Options: None, EntraID
)

# Verify administrator
if (-NOT ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole] "Administrator")) {
    Write-Error "This script must be run as Administrator"
    exit 1
}

# Check if WinGet is installed
try {
    $wingetVersion = winget --version
    Write-Host "WinGet version: $wingetVersion" -ForegroundColor Green
} catch {
    Write-Error "WinGet is not installed. Install from Microsoft Store: ms-windows-store://pdp/?ProductId=9NBLGGH4NNS1"
    exit 1
}

# Remove existing source if present
$existingSources = winget source list | Out-String
if ($existingSources -match $SourceName) {
    Write-Host "Removing existing source '$SourceName'..." -ForegroundColor Yellow
    winget source remove --name $SourceName
    Start-Sleep -Seconds 2
}

# Add source
Write-Host "Adding source '$SourceName'..." -ForegroundColor Cyan

$addCommand = "winget source add --name `"$SourceName`" --arg `"$SourceUrl`" --type `"Microsoft.Rest`" --accept-source-agreements"

if ($AuthType -eq "EntraID") {
    $addCommand += " --authorization EntraID"
}

Invoke-Expression $addCommand

# Verify
Write-Host "`nVerifying source configuration..." -ForegroundColor Cyan
winget source list --name $SourceName

# Update source
Write-Host "`nUpdating source data..." -ForegroundColor Cyan
winget source update --name $SourceName

Write-Host "`nSource '$SourceName' configured successfully!" -ForegroundColor Green
Write-Host "Users can now install packages with: winget install PackageName --source $SourceName"
```

**Deploy via Group Policy or Intune:**

```powershell
# GPO Startup Script or Intune PowerShell Script
Invoke-WebRequest -Uri "https://internal.company.com/scripts/Add-CompanyWinGetSource.ps1" -OutFile "$env:TEMP\Add-Source.ps1"
& "$env:TEMP\Add-Source.ps1"
```

### URL-based access without per-user authentication

For scenarios where you want "anyone with the URL" to access:

**Approach 1: Network-level security**
- Deploy REST source on internal network only
- Use VPN or private network access
- No authentication at WinGet level
- Firewall provides access control

**Approach 2: Reverse proxy with IP allowlisting**
```nginx
# nginx configuration
server {
    listen 443 ssl;
    server_name packages.company.com;
    
    # IP allowlist
    allow 10.0.0.0/8;     # Internal network
    allow 203.0.113.0/24; # VPN range
    deny all;
    
    location /api/ {
        proxy_pass http://winget-backend:8080/;
        proxy_set_header Host $host;
    }
}
```

**Approach 3: Shared token in URL** (less secure)
- Include authentication in URL structure
- Configure source: `https://packages.company.com/api/?token=shared-secret`
- **Warning**: URLs may be logged; use only for low-security scenarios

## Part 4: Complete Implementation Example

Let's walk through a complete real-world implementation.

### Scenario: Private company application

**Requirements:**
- Private WinGet repository for internal tools
- Automated publishing on release
- Accessible to company employees only
- Hosted on company infrastructure (not Azure)

### Step 1: Set up self-hosted WinGetty server

```bash
# On company server (Linux)
git clone https://github.com/thilojaeggi/WinGetty
cd WinGetty

# Configure
cat > .env << EOF
SECRET_KEY=$(openssl rand -hex 32)
DATABASE_URL=postgresql://user:pass@localhost/wingetty
WINGETTY_ADMIN_PASSWORD=$(openssl rand -base64 16)
WINGETTY_URL=https://packages.company.com
EOF

# Deploy
docker-compose up -d

# Configure reverse proxy (nginx)
cat > /etc/nginx/sites-available/winget << EOF
server {
    listen 443 ssl http2;
    server_name packages.company.com;
    
    ssl_certificate /etc/ssl/certs/company.crt;
    ssl_certificate_key /etc/ssl/private/company.key;
    
    location / {
        proxy_pass http://localhost:8080;
        proxy_set_header Host \$host;
        proxy_set_header X-Real-IP \$remote_addr;
    }
}
EOF

ln -s /etc/nginx/sites-available/winget /etc/nginx/sites-enabled/
nginx -t && systemctl reload nginx
```

### Step 2: Create GitHub repository structure

```
company-internal-tool/
├── .github/
│   └── workflows/
│       ├── build.yml           # Build installer
│       └── publish-winget.yml  # Publish to WinGetty
├── src/
│   └── (application source)
├── packaging/
│   ├── wix/                    # WiX installer config
│   └── manifests/
│       └── template.yaml       # Manifest template
└── README.md
```

### Step 3: GitHub Actions workflow

`.github/workflows/build.yml`:
```yaml
name: Build and Release

on:
  push:
    tags:
      - 'v*'

jobs:
  build:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4
      
      - name: Setup .NET
        uses: actions/setup-dotnet@v4
        with:
          dotnet-version: '8.0.x'
      
      - name: Build Application
        run: dotnet build -c Release
      
      - name: Build MSI Installer
        run: |
          # Build with WiX
          dotnet tool install --global wix
          wix build packaging/wix/Product.wxs -o CompanyTool.msi
      
      - name: Create Release
        uses: softprops/action-gh-release@v1
        with:
          files: CompanyTool.msi
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
      
      - name: Trigger WinGet Publish
        uses: peter-evans/repository-dispatch@v3
        with:
          event-type: winget-publish
          client-payload: |
            {
              "version": "${{ github.ref_name }}",
              "installer_url": "https://github.com/${{ github.repository }}/releases/download/${{ github.ref_name }}/CompanyTool.msi"
            }
```

`.github/workflows/publish-winget.yml`:
```yaml
name: Publish to Company WinGet

on:
  repository_dispatch:
    types: [winget-publish]

jobs:
  publish:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      
      - name: Download Installer
        run: |
          wget -O installer.msi "${{ github.event.client_payload.installer_url }}"
      
      - name: Calculate SHA256
        id: hash
        run: |
          HASH=$(sha256sum installer.msi | cut -d' ' -f1)
          echo "sha256=$HASH" >> $GITHUB_OUTPUT
      
      - name: Create Manifest
        run: |
          VERSION="${{ github.event.client_payload.version }}"
          VERSION="${VERSION#v}"  # Strip 'v' prefix
          
          mkdir -p manifests/Company/InternalTool/$VERSION
          
          # Create manifests using template
          cat > manifests/Company/InternalTool/$VERSION/Company.InternalTool.yaml << EOF
          PackageIdentifier: Company.InternalTool
          PackageVersion: $VERSION
          DefaultLocale: en-US
          ManifestType: version
          ManifestVersion: 1.6.0
          EOF
          
          cat > manifests/Company/InternalTool/$VERSION/Company.InternalTool.installer.yaml << EOF
          PackageIdentifier: Company.InternalTool
          PackageVersion: $VERSION
          InstallerType: msi
          Scope: machine
          InstallModes:
            - interactive
            - silent
          InstallerSwitches:
            Silent: /qn
            SilentWithProgress: /qb
          UpgradeBehavior: install
          Installers:
            - Architecture: x64
              InstallerUrl: ${{ github.event.client_payload.installer_url }}
              InstallerSha256: ${{ steps.hash.outputs.sha256 }}
          ManifestType: installer
          ManifestVersion: 1.6.0
          EOF
          
          cat > manifests/Company/InternalTool/$VERSION/Company.InternalTool.locale.en-US.yaml << EOF
          PackageIdentifier: Company.InternalTool
          PackageVersion: $VERSION
          PackageLocale: en-US
          Publisher: Company Name
          PublisherUrl: https://www.company.com
          PublisherSupportUrl: https://support.company.com
          PrivacyUrl: https://www.company.com/privacy
          PackageName: Internal Tool
          PackageUrl: https://tools.company.com/internal-tool
          License: Proprietary
          ShortDescription: Company internal productivity tool
          Description: |
            Detailed description of your internal tool.
            Multiple lines supported.
          Moniker: internal-tool
          Tags:
            - internal
            - productivity
            - company-tool
          ManifestType: defaultLocale
          ManifestVersion: 1.6.0
          EOF
      
      - name: Validate Manifests
        run: |
          # Install winget (on Ubuntu runner)
          sudo add-apt-repository ppa:git-core/ppa
          sudo apt update
          sudo apt install -y wget
          
          # Validate YAML syntax
          pip install pyyaml
          python -c "
          import yaml
          import sys
          from pathlib import Path
          
          manifest_dir = Path('manifests/Company/InternalTool/${{ github.event.client_payload.version }}'.replace('v', ''))
          for file in manifest_dir.glob('*.yaml'):
              try:
                  with open(file) as f:
                      yaml.safe_load(f)
                  print(f'✓ {file.name} is valid')
              except Exception as e:
                  print(f'✗ {file.name} is invalid: {e}')
                  sys.exit(1)
          "
      
      - name: Publish to WinGetty
        run: |
          # Upload manifests to WinGetty API
          for file in manifests/Company/InternalTool/${{ github.event.client_payload.version }}/*.yaml; do
            echo "Uploading $file..."
            curl -X POST \
              -H "Content-Type: application/json" \
              -H "Authorization: Bearer ${{ secrets.WINGETTY_API_KEY }}" \
              -d @"$file" \
              https://packages.company.com/api/packageManifests
          done
```

### Step 4: Deploy to client machines

Create deployment package for IT:

```powershell
# Deploy-CompanyWinGet.ps1
# Run via Intune or Group Policy

# 1. Ensure WinGet is installed
if (!(Get-Command winget -ErrorAction SilentlyContinue)) {
    Write-Host "Installing WinGet..."
    Add-AppxPackage -Path "https://aka.ms/getwinget"
}

# 2. Add company source
winget source add `
    --name "CompanyApps" `
    --arg "https://packages.company.com/api/" `
    --type "Microsoft.Rest" `
    --accept-source-agreements

# 3. Install company tool
winget install Company.InternalTool --source CompanyApps --silent --accept-package-agreements

Write-Host "Company WinGet source configured and Internal Tool installed successfully!"
```

## Part 5: Manifest Creation Best Practices

### Creating high-quality manifests

**Required manifest fields:**
```yaml
# Minimal valid manifest structure

# Version manifest (Package.yaml)
PackageIdentifier: Publisher.Product
PackageVersion: 1.0.0
DefaultLocale: en-US
ManifestType: version
ManifestVersion: 1.6.0

# Installer manifest (Package.installer.yaml)
PackageIdentifier: Publisher.Product
PackageVersion: 1.0.0
InstallerType: msi  # Options: msi, exe, msix, appx, inno, wix, nullsoft, burn, portable
Installers:
  - Architecture: x64  # Options: x86, x64, arm, arm64, neutral
    InstallerUrl: https://example.com/installer.msi
    InstallerSha256: ABC123...  # 64-character SHA256 hash
ManifestType: installer
ManifestVersion: 1.6.0

# Locale manifest (Package.locale.en-US.yaml)
PackageIdentifier: Publisher.Product
PackageVersion: 1.0.0
PackageLocale: en-US
Publisher: Publisher Name
PackageName: Product Name
License: MIT
ShortDescription: Brief description under 256 characters
ManifestType: defaultLocale
ManifestVersion: 1.6.0
```

### Generate SHA256 hashes

```powershell
# PowerShell
$hash = (Get-FileHash -Path "installer.msi" -Algorithm SHA256).Hash
Write-Host $hash

# Or use winget hash command
winget hash installer.msi
```

### Installer type detection

| Installer Type | Silent Switch | Progress Switch | File Extension |
|---------------|---------------|-----------------|----------------|
| MSI | `/quiet` | `/passive` | .msi |
| Inno Setup | `/VERYSILENT` | `/SILENT` | .exe (Inno) |
| Nullsoft (NSIS) | `/S` | `/S` | .exe (NSIS) |
| WiX Burn | `/quiet` | `/passive` | .exe (Burn) |
| MSIX/APPX | N/A | N/A | .msix, .appx |
| Portable | N/A | N/A | .exe, .zip |

### Advanced installer configuration

```yaml
InstallerType: exe
InstallerSwitches:
  Silent: /S /SILENT
  SilentWithProgress: /S /PROGRESS
  Interactive: /I
  InstallLocation: /DIR="<INSTALLPATH>"
  Log: /LOG="<LOGPATH>"
  Upgrade: /UPGRADE
  Custom: /CustomParameter=Value
Scope: machine  # or 'user'
InstallModes:
  - interactive
  - silent
  - silentWithProgress
UpgradeBehavior: install  # or 'uninstallPrevious'
Commands:
  - yourapp
  - yt  # Command aliases
FileExtensions:
  - pdf
  - docx
Protocols:
  - yourapp
  - yt
InstallerSuccessCodes:
  - 0
  - 3010  # Reboot required
ExpectedReturnCodes:
  - InstallerReturnCode: 1641
    ReturnResponse: installInProgress
  - InstallerReturnCode: 3010
    ReturnResponse: rebootRequiredToFinish
```

## Part 6: Common Issues and Troubleshooting

### Issue 1: "Failed to add source - certificate not trusted"

**Symptoms:**
```
Failed to add source: 0x801901f4
```

**Solution:**
```powershell
# Import certificate to Trusted Root
$cert = New-Object System.Security.Cryptography.X509Certificates.X509Certificate2
$cert.Import("path\to\certificate.crt")
$store = New-Object System.Security.Cryptography.X509Certificates.X509Store("Root", "LocalMachine")
$store.Open("ReadWrite")
$store.Add($cert)
$store.Close()
```

### Issue 2: GitHub Actions workflow not triggering

**Cause:** Release is in draft state

**Solution:**
- Ensure release is **published** (not draft)
- Check workflow trigger: `types: [published]`
- Draft releases don't trigger workflows by design

### Issue 3: Manifest validation failures

**Common errors:**

```yaml
# ❌ WRONG - Missing required field
PackageIdentifier: Company.App
PackageVersion: 1.0.0
# Missing other required fields

# ✅ CORRECT - All required fields
PackageIdentifier: Company.App
PackageVersion: 1.0.0
DefaultLocale: en-US
ManifestType: version
ManifestVersion: 1.6.0
```

**Validation command:**
```bash
# Validate locally before submitting
winget validate path/to/manifests/Company/App/1.0.0/
```

### Issue 4: SHA256 hash mismatch

**Cause:** Installer file changed after manifest creation

**Solution:**
```powershell
# Always regenerate hash
$hash = (Get-FileHash -Path "installer.msi" -Algorithm SHA256).Hash

# Update manifest
# InstallerSha256: $hash
```

### Issue 5: WinGet can't find package in private source

**Diagnostic steps:**
```bash
# 1. Verify source is added
winget source list

# 2. Update source cache
winget source update --name CompanyRepo

# 3. Search explicitly in source
winget search --source CompanyRepo

# 4. Try with exact package ID
winget show Company.App --source CompanyRepo
```

### Issue 6: Authentication failures with private source

**For Entra ID sources:**
```bash
# Check version supports Entra ID
winget --version  # Need v1.7.10582+

# Remove and re-add source
winget source remove --name CompanyRepo
winget source add --name CompanyRepo --arg https://packages.company.com/api/ --type Microsoft.Rest --authorization EntraID
```

## Part 7: Best Practices and Recommendations

### Security best practices

1. **Use HTTPS exclusively**
   - WinGet requires HTTPS for all sources
   - Obtain certificates from trusted CA
   - Avoid self-signed certificates in production

2. **Implement authentication for private sources**
   - Use Microsoft Entra ID for enterprise scenarios
   - Enable per-user or per-group access control
   - Audit access logs regularly

3. **Protect API keys and tokens**
   - Store in GitHub Secrets, never in code
   - Rotate regularly (every 90 days)
   - Use dedicated bot accounts for automation
   - Apply principle of least privilege

4. **Validate installers before publishing**
   - Run antivirus scans
   - Test in Windows Sandbox
   - Verify digital signatures
   - Check SmartScreen reputation

5. **Monitor manifest submissions**
   - Review automated PRs before merge
   - Implement approval workflows
   - Track changes to manifests

### Operational best practices

1. **Version management**
   - Use semantic versioning (SemVer)
   - Limit versions in repository (set `max-versions-to-keep`)
   - Document breaking changes

2. **Release process**
   - Always create GitHub releases (not just tags)
   - Attach installer binaries to releases
   - Include release notes
   - Publish releases (don't leave as drafts)

3. **Manifest quality**
   - Include comprehensive metadata
   - Provide accurate descriptions
   - Add relevant tags for searchability
   - Include support URLs
   - Specify moniker for easy installation

4. **Testing workflow**
   - Test workflows with `workflow_dispatch` before automating
   - Validate manifests locally first
   - Use staging releases for testing
   - Monitor PR validation results

5. **Client deployment**
   - Use Group Policy or Intune for source configuration
   - Include source setup in onboarding process
   - Document installation instructions
   - Provide helpdesk support resources

### Performance considerations

1. **Source update frequency**
   ```json
   {
     "source": {
       "autoUpdateIntervalInMinutes": 60  // Balance freshness vs network load
     }
   }
   ```

2. **Installer hosting**
   - Use CDN for public packages
   - Host installers close to users geographically
   - Implement caching strategies
   - Monitor download bandwidth

3. **Database optimization** (for self-hosted)
   - Index package identifiers
   - Implement pagination for large repositories
   - Use appropriate database (PostgreSQL for scale)
   - Regular database maintenance

### Maintenance and monitoring

1. **Monitor automation health**
   - Set up GitHub Actions notifications
   - Track workflow success rates
   - Review failed runs promptly
   - Update actions dependencies regularly

2. **Track package usage**
   - Log client requests to private sources
   - Monitor popular packages
   - Identify unused packages for removal
   - Plan capacity based on usage trends

3. **Keep tooling updated**
   - Update WinGetCreate regularly
   - Monitor winget-cli releases
   - Update GitHub Actions to latest versions
   - Review manifest schema updates

4. **Documentation maintenance**
   - Document internal processes
   - Maintain runbooks for common issues
   - Keep client setup instructions current
   - Document API changes

## Part 8: Limitations and Considerations

### WinGet limitations

1. **Windows version requirements**
   - Windows 10 version 1809 (build 17763) or later
   - Windows 11 (all versions)
   - Windows Server 2022+ (Server 2019 not officially supported)

2. **Administrator privileges**
   - Required to add/remove sources
   - Required for machine-scope installations
   - Consider security implications

3. **Source synchronization**
   - REST sources require periodic updates
   - Network connectivity required
   - Sync failures can prevent installations

4. **Package size limits**
   - No hard limit on installer size
   - Large installers may timeout
   - Consider network bandwidth constraints

### GitHub-specific considerations

1. **GitHub Actions quotas**
   - **Public repos**: Unlimited minutes
   - **Private repos**: 2,000 minutes/month (free tier)
   - Additional minutes can be purchased
   - Consider workflow efficiency

2. **Release asset size limits**
   - Maximum file size: 2 GB per file
   - Total release size: No documented limit
   - Consider splitting large installers

3. **API rate limits**
   - Authenticated: 5,000 requests/hour
   - Unauthenticated: 60 requests/hour
   - Impacts automated workflows

### Private source hosting considerations

**Azure costs** (Microsoft official solution):
- Developer: ~$10/month
- Basic: ~$75/month
- Enhanced: ~$200+/month
- Cosmos DB consumption varies with usage

**Self-hosted requirements:**
- Linux or Windows server
- 2+ CPU cores recommended
- 4+ GB RAM recommended
- 50+ GB storage (scales with packages)
- HTTPS certificate
- Domain name

**Bandwidth considerations:**
- Each install downloads full installer
- Plan for peak usage (Monday mornings, onboarding)
- Consider CDN for geographically distributed users
- Monitor and set alerts for bandwidth limits

### Manifest compatibility

1. **Schema versioning**
   - Use latest schema version (1.6.0 as of 2025)
   - Older WinGet versions may not support new features
   - Test with minimum supported version

2. **Cross-platform considerations**
   - WinGet is Windows-only
   - ARM64 support available
   - Test on all target architectures

## Summary and Quick Reference

### Quick decision tree

**Choose Option 1 (Public winget-pkgs)** if:
- ✅ Your software is open-source or publicly distributable
- ✅ You want maximum user reach
- ✅ You're comfortable with public submission process
- ✅ You can meet Microsoft's submission requirements

**Choose Option 2 (Private REST source)** if:
- ✅ Your software is proprietary/internal
- ✅ You need access control
- ✅ You have infrastructure for hosting (Azure or self-hosted)
- ✅ You need guaranteed package availability

### Essential commands cheat sheet

```bash
# Source management
winget source list
winget source add --name NAME --arg URL --type Microsoft.Rest
winget source remove --name NAME
winget source update --name NAME
winget source reset --force

# Package operations
winget search QUERY
winget search QUERY --source SOURCE
winget show PACKAGEID
winget install PACKAGEID
winget install PACKAGEID --source SOURCE
winget upgrade PACKAGEID
winget uninstall PACKAGEID

# Manifest tools
winget validate PATH
winget hash INSTALLER
wingetcreate new URL
wingetcreate update PACKAGEID -v VERSION -u URL -t TOKEN --submit

# Settings
winget settings
```

### Required secrets for automation

**For public winget-pkgs:**
- `WINGET_TOKEN`: GitHub PAT with `public_repo` scope

**For private source:**
- `PRIVATE_SOURCE_URL`: Your REST API endpoint
- `PRIVATE_SOURCE_KEY`: API authentication key
- `WINGETTY_API_KEY`: WinGetty specific (if using)

### Key file locations

```
# WinGet settings
%LOCALAPPDATA%\Packages\Microsoft.DesktopAppInstaller_8wekyb3d8bbwe\LocalState\settings.json

# WinGet cache
%LOCALAPPDATA%\Packages\Microsoft.DesktopAppInstaller_8wekyb3d8bbwe\LocalCache\

# Manifest structure
manifests/
└── {first-letter}/
    └── {Publisher}/
        └── {Application}/
            └── {Version}/
                ├── {Publisher}.{Application}.yaml
                ├── {Publisher}.{Application}.installer.yaml
                └── {Publisher}.{Application}.locale.en-US.yaml
```

### Additional resources

**Official documentation:**
- WinGet documentation: https://learn.microsoft.com/windows/package-manager/
- Winget-pkgs repository: https://github.com/microsoft/winget-pkgs
- WinGetCreate tool: https://github.com/microsoft/winget-create
- REST source reference: https://github.com/microsoft/winget-cli-restsource

**Community tools:**
- WinGet Releaser action: https://github.com/vedantmgoyal9/winget-releaser
- WinGetty (self-hosted): https://github.com/thilojaeggi/WinGetty
- winget.pro (commercial): https://winget.pro
- Komac (manifest creator): https://github.com/russellbanks/komac

**Support:**
- WinGet CLI issues: https://github.com/microsoft/winget-cli/issues
- Community discussions: https://github.com/microsoft/winget-pkgs/discussions

This comprehensive guide provides everything needed to implement WinGet package distribution using GitHub infrastructure, whether through automated publishing to the public repository or setting up private sources with full automation via GitHub Actions.