#!/bin/bash
# =============================================================================
# ZeroClaw Secure Fork - Automated Deployment Script
# =============================================================================
# This script deploys ZeroClaw with maximum security hardening on Ubuntu.
#
# Usage:
#   sudo ./deploy.sh [options]
#
# Options:
#   --minimal       Skip optional dependencies (firejail, bubblewrap)
#   --no-build      Skip building from source (use existing binary)
#   --no-service    Skip systemd service installation
#   --uninstall     Remove ZeroClaw installation
#   --verify-only   Only run security verification
#   --help          Show this help message
#
# =============================================================================

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
ZEROCLAW_USER="zeroclaw"
ZEROCLAW_GROUP="zeroclaw"
INSTALL_DIR="/opt/zeroclaw"
CONFIG_DIR="/etc/zeroclaw"
DATA_DIR="/var/lib/zeroclaw"
LOG_DIR="/var/log/zeroclaw"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Flags
MINIMAL=false
NO_BUILD=false
NO_SERVICE=false
UNINSTALL=false
VERIFY_ONLY=false

# -----------------------------------------------------------------------------
# Helper Functions
# -----------------------------------------------------------------------------

print_banner() {
    echo -e "${BLUE}"
    echo "=============================================="
    echo "  ZeroClaw Secure Fork - Deployment Script"
    echo "=============================================="
    echo -e "${NC}"
}

log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

log_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

log_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

check_root() {
    if [[ $EUID -ne 0 ]]; then
        log_error "This script must be run as root (use sudo)"
        exit 1
    fi
}

detect_ubuntu_version() {
    if [ -f /etc/os-release ]; then
        . /etc/os-release
        OS_NAME=$NAME
        OS_VERSION=$VERSION_ID
        log_info "Detected: $OS_NAME $OS_VERSION"
        
        # Check if Ubuntu 20.04+
        if [[ "$ID" == "ubuntu" ]]; then
            if [[ "${OS_VERSION%%.*}" -lt 20 ]]; then
                log_warning "Ubuntu 20.04+ recommended. You have $OS_VERSION"
            fi
        elif [[ "$ID" == "debian" ]]; then
            if [[ "${OS_VERSION%%.*}" -lt 11 ]]; then
                log_warning "Debian 11+ recommended. You have $OS_VERSION"
            fi
        else
            log_warning "This script is designed for Ubuntu/Debian. Proceed with caution."
        fi
    else
        log_warning "Cannot detect OS version. Proceeding anyway..."
    fi
}

check_dependencies() {
    log_info "Checking dependencies..."
    
    local missing_deps=()
    
    # Essential dependencies
    for cmd in curl git; do
        if ! command -v $cmd &> /dev/null; then
            missing_deps+=($cmd)
        fi
    done
    
    if [ ${#missing_deps[@]} -ne 0 ]; then
        log_info "Installing missing dependencies: ${missing_deps[*]}"
        apt-get update -qq
        apt-get install -y "${missing_deps[@]}"
    fi
    
    log_success "Dependencies check passed"
}

install_rust() {
    if ! command -v cargo &> /dev/null; then
        log_info "Installing Rust toolchain..."
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
        source "$HOME/.cargo/env"
        log_success "Rust installed"
    else
        log_info "Rust already installed: $(cargo --version)"
    fi
}

install_build_deps() {
    log_info "Installing build dependencies..."
    apt-get update -qq
    apt-get install -y build-essential pkg-config libssl-dev
    log_success "Build dependencies installed"
}

install_optional_deps() {
    if [ "$MINIMAL" = false ]; then
        log_info "Installing optional security tools..."
        apt-get install -y firejail bubblewrap 2>/dev/null || {
            log_warning "Some optional deps failed to install (non-critical)"
        }
        log_success "Optional security tools installed"
    fi
}

# -----------------------------------------------------------------------------
# Installation Functions
# -----------------------------------------------------------------------------

create_user() {
    log_info "Creating zeroclaw user..."
    
    if id "$ZEROCLAW_USER" &>/dev/null; then
        log_info "User $ZEROCLAW_USER already exists"
    else
        useradd -r -s /bin/false -d "$DATA_DIR" "$ZEROCLAW_USER"
        log_success "User $ZEROCLAW_USER created"
    fi
}

create_directories() {
    log_info "Creating directories..."
    
    mkdir -p "$INSTALL_DIR"
    mkdir -p "$CONFIG_DIR"
    mkdir -p "$CONFIG_DIR/secrets"
    mkdir -p "$DATA_DIR/workspace"
    mkdir -p "$DATA_DIR/backups"
    mkdir -p "$DATA_DIR/keys"
    mkdir -p "$LOG_DIR"
    
    # Set permissions
    chown -R "$ZEROCLAW_USER:$ZEROCLAW_GROUP" "$DATA_DIR"
    chown -R root:$ZEROCLAW_GROUP "$CONFIG_DIR"
    chmod 750 "$CONFIG_DIR"
    chmod 700 "$CONFIG_DIR/secrets"
    chmod 755 "$DATA_DIR/workspace"
    chmod 750 "$DATA_DIR/backups"
    chmod 700 "$DATA_DIR/keys"
    chown -R "$ZEROCLAW_USER:$ZEROCLAW_GROUP" "$LOG_DIR"
    chmod 750 "$LOG_DIR"
    
    log_success "Directories created with secure permissions"
}

build_zeroclaw() {
    if [ "$NO_BUILD" = true ]; then
        log_info "Skipping build (--no-build flag)"
        return
    fi
    
    log_info "Building ZeroClaw from source..."
    
    # Ensure Rust is available
    if [ -f "$HOME/.cargo/env" ]; then
        source "$HOME/.cargo/env"
    fi
    
    cd "$SCRIPT_DIR"
    cargo build --release
    
    log_success "ZeroClaw built successfully"
}

install_binary() {
    log_info "Installing ZeroClaw binary..."
    
    if [ -f "$SCRIPT_DIR/target/release/zeroclaw" ]; then
        cp "$SCRIPT_DIR/target/release/zeroclaw" "$INSTALL_DIR/"
        chmod 755 "$INSTALL_DIR/zeroclaw"
        log_success "Binary installed to $INSTALL_DIR/zeroclaw"
    else
        log_error "Binary not found. Run without --no-build or build manually."
        exit 1
    fi
}

install_config() {
    log_info "Installing hardened configuration..."
    
    # Copy hardened config
    cp "$SCRIPT_DIR/security/hardened_config.toml" "$CONFIG_DIR/config.toml"
    chmod 640 "$CONFIG_DIR/config.toml"
    chown root:$ZEROCLAW_GROUP "$CONFIG_DIR/config.toml"
    
    # Copy allowed commands
    cp "$SCRIPT_DIR/security/allowed_commands.txt" "$CONFIG_DIR/"
    chmod 640 "$CONFIG_DIR/allowed_commands.txt"
    chown root:$ZEROCLAW_GROUP "$CONFIG_DIR/allowed_commands.txt"
    
    log_success "Configuration installed"
}

install_systemd_service() {
    if [ "$NO_SERVICE" = true ]; then
        log_info "Skipping systemd service (--no-service flag)"
        return
    fi
    
    log_info "Installing systemd service..."
    
    cp "$SCRIPT_DIR/security/systemd/zeroclaw-hardened.service" /etc/systemd/system/zeroclaw.service
    systemctl daemon-reload
    
    log_success "Systemd service installed"
}

enable_service() {
    if [ "$NO_SERVICE" = true ]; then
        return
    fi
    
    log_info "Enabling ZeroClaw service..."
    systemctl enable zeroclaw
    log_success "Service enabled"
}

start_service() {
    if [ "$NO_SERVICE" = true ]; then
        return
    fi
    
    read -p "Start ZeroClaw service now? (y/n) " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        systemctl start zeroclaw
        sleep 2
        if systemctl is-active --quiet zeroclaw; then
            log_success "Service started successfully"
        else
            log_error "Service failed to start. Check: journalctl -u zeroclaw"
        fi
    fi
}

# -----------------------------------------------------------------------------
# Security Verification
# -----------------------------------------------------------------------------

run_security_verification() {
    log_info "Running security verification..."
    
    if [ -x "$SCRIPT_DIR/security/security_verify.sh" ]; then
        "$SCRIPT_DIR/security/security_verify.sh"
    else
        log_warning "Security verification script not found or not executable"
    fi
}

check_systemd_security() {
    if [ "$NO_SERVICE" = false ] && systemctl is-active --quiet zeroclaw; then
        log_info "Checking systemd security score..."
        local score=$(systemd-analyze security zeroclaw 2>/dev/null | grep "Overall exposure" | awk '{print $NF}')
        
        if [ -n "$score" ]; then
            echo -e "Systemd security score: ${YELLOW}${score}${NC}"
            
            # Parse score (remove UNSAFE/OK suffix)
            local numeric_score=$(echo "$score" | grep -oP '[\d.]+')
            if (( $(echo "$numeric_score < 2.0" | bc -l) )); then
                log_success "Excellent security score!"
            elif (( $(echo "$numeric_score < 3.0" | bc -l) )); then
                log_success "Good security score"
            else
                log_warning "Security score could be improved"
            fi
        fi
    fi
}

# -----------------------------------------------------------------------------
# Uninstall
# -----------------------------------------------------------------------------

uninstall() {
    log_warning "Uninstalling ZeroClaw..."
    
    read -p "This will remove ZeroClaw. Continue? (y/n) " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        log_info "Uninstall cancelled"
        exit 0
    fi
    
    # Stop and disable service
    systemctl stop zeroclaw 2>/dev/null || true
    systemctl disable zeroclaw 2>/dev/null || true
    rm -f /etc/systemd/system/zeroclaw.service
    systemctl daemon-reload
    
    # Ask about data preservation
    read -p "Remove data directories ($DATA_DIR)? (y/n) " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        rm -rf "$DATA_DIR"
        log_info "Data directories removed"
    else
        log_info "Data directories preserved"
    fi
    
    # Remove installation
    rm -rf "$INSTALL_DIR"
    rm -rf "$CONFIG_DIR"
    rm -rf "$LOG_DIR"
    
    # Remove user
    read -p "Remove zeroclaw user? (y/n) " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        userdel "$ZEROCLAW_USER" 2>/dev/null || true
        log_info "User removed"
    fi
    
    log_success "ZeroClaw uninstalled"
}

# -----------------------------------------------------------------------------
# Main
# -----------------------------------------------------------------------------

show_help() {
    cat << EOF
ZeroClaw Secure Fork - Deployment Script

Usage: sudo ./deploy.sh [options]

Options:
  --minimal       Skip optional dependencies (firejail, bubblewrap)
  --no-build      Skip building from source (use existing binary)
  --no-service    Skip systemd service installation
  --uninstall     Remove ZeroClaw installation
  --verify-only   Only run security verification
  --help          Show this help message

Examples:
  sudo ./deploy.sh                 # Full installation
  sudo ./deploy.sh --minimal       # Minimal installation
  sudo ./deploy.sh --verify-only   # Just run security checks
  sudo ./deploy.sh --uninstall     # Remove installation
EOF
}

parse_args() {
    while [[ $# -gt 0 ]]; do
        case $1 in
            --minimal)
                MINIMAL=true
                shift
                ;;
            --no-build)
                NO_BUILD=true
                shift
                ;;
            --no-service)
                NO_SERVICE=true
                shift
                ;;
            --uninstall)
                UNINSTALL=true
                shift
                ;;
            --verify-only)
                VERIFY_ONLY=true
                shift
                ;;
            --help|-h)
                show_help
                exit 0
                ;;
            *)
                log_error "Unknown option: $1"
                show_help
                exit 1
                ;;
        esac
    done
}

main() {
    parse_args "$@"
    print_banner
    check_root
    
    if [ "$UNINSTALL" = true ]; then
        uninstall
        exit 0
    fi
    
    if [ "$VERIFY_ONLY" = true ]; then
        run_security_verification
        check_systemd_security
        exit 0
    fi
    
    # Detection and checks
    detect_ubuntu_version
    check_dependencies
    
    # Installation
    install_build_deps
    install_rust
    install_optional_deps
    
    create_user
    create_directories
    
    build_zeroclaw
    install_binary
    install_config
    install_systemd_service
    enable_service
    
    # Verification
    run_security_verification
    start_service
    check_systemd_security
    
    # Post-install instructions
    echo ""
    echo -e "${GREEN}=========================================="
    echo "  Installation Complete!"
    echo "==========================================${NC}"
    echo ""
    echo "Next steps:"
    echo "  1. Configure your API key:"
    echo "     sudo nano /etc/zeroclaw/secrets/api_key"
    echo ""
    echo "  2. Review the configuration:"
    echo "     sudo nano /etc/zeroclaw/config.toml"
    echo ""
    echo "  3. Start the service (if not already started):"
    echo "     sudo systemctl start zeroclaw"
    echo ""
    echo "  4. Check status:"
    echo "     sudo systemctl status zeroclaw"
    echo "     journalctl -u zeroclaw -f"
    echo ""
    echo "  5. Run security verification anytime:"
    echo "     ./security/security_verify.sh"
    echo ""
    echo "Documentation:"
    echo "  - HARDENING_GUIDE.md"
    echo "  - DEPLOYMENT_CHECKLIST.md"
    echo "  - security/DEPLOYMENT_GUIDE.md"
    echo ""
}

main "$@"
