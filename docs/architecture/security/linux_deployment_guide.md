# ZeroClaw Linux Deployment — Fresh OS to Production

**Version:** 1.0  
**Date:** 2025-04-14  
**Status:** Production Deployment Guide  
**Audience:** DevOps engineers, system administrators, infrastructure teams

---

## Executive Summary

This guide provides step-by-step instructions to deploy ZeroClaw from a fresh Linux installation to a fully hardened, production-ready state. Starting with a minimal Ubuntu 22.04 LTS or Debian 12 image, this guide covers OS hardening, Docker setup, network configuration, secret management, and deployment of ZeroClaw containers.

**Estimated Deployment Time:** 30-45 minutes  
**Prerequisite Knowledge:** Linux administration, Docker, networking basics  
**Hardware Requirements:** 2+ vCPU, 4GB+ RAM, 20GB+ disk  

---

## 1. OS Installation & Initial Setup

### 1.1 Fresh OS Installation

Start with:
- **Ubuntu 22.04 LTS** (Long-Term Support) OR
- **Debian 12 (Bookworm)**

Both are available from cloud providers (AWS EC2, Google Compute, Azure) or downloadable ISOs.

```bash
# After OS installation, update system packages
sudo apt-get update
sudo apt-get upgrade -y

# Install essential tools
sudo apt-get install -y \
    curl \
    wget \
    git \
    vim \
    htop \
    net-tools \
    build-essential \
    openssl \
    openssh-server \
    openssh-client

# Verify OpenSSH is running
sudo systemctl status ssh
```

### 1.2 User Setup (Non-root Execution)

ZeroClaw must NOT run as root. Create a dedicated user:

```bash
#!/bin/bash
# setup-zeroclaw-user.sh

# Create zeroclaw user (no login shell)
sudo useradd -m -u 1000 -s /bin/bash zeroclaw

# Create required directories
sudo mkdir -p /zeroclaw-data/{workspace,models,logs,config}
sudo chown -R zeroclaw:zeroclaw /zeroclaw-data
sudo chmod 750 /zeroclaw-data
sudo chmod 750 /zeroclaw-data/*

# Add zeroclaw to docker group (so it can run containers)
sudo usermod -aG docker zeroclaw

# Verify user
sudo su - zeroclaw -c "echo 'User zeroclaw created'"
```

### 1.3 SSH Hardening

```bash
#!/bin/bash
# harden-ssh.sh

# Backup original config
sudo cp /etc/ssh/sshd_config /etc/ssh/sshd_config.backup

# Apply hardening settings
sudo tee -a /etc/ssh/sshd_config > /dev/null <<EOF

# Security hardening
PermitRootLogin no                    # Disable root SSH
PasswordAuthentication no              # SSH keys only
PubkeyAuthentication yes
PermitEmptyPasswords no
X11Forwarding no
MaxAuthTries 3
MaxSessions 5
ClientAliveInterval 300                # Disconnect idle clients
ClientAliveCountMax 0
Protocol 2                             # SSH v2 only
Ciphers chacha20-poly1305@openssh.com,aes256-gcm@openssh.com
MACs hmac-sha2-512-etm@openssh.com
KexAlgorithms curve25519-sha256
HostKey /etc/ssh/ssh_host_ed25519_key
EOF

# Test config syntax
sudo sshd -t

# Restart SSH
sudo systemctl restart ssh

echo "[✓] SSH hardening applied"
```

---

## 2. Firewall Configuration

### 2.1 UFW (Uncomplicated Firewall) Setup

```bash
#!/bin/bash
# setup-firewall.sh

set -e

echo "[*] Setting up UFW firewall..."

# Enable UFW
sudo ufw default deny incoming
sudo ufw default allow outgoing
sudo ufw default allow routed

# SSH access (critical!)
sudo ufw allow 22/tcp comment "SSH access"

# ZeroClaw API server
sudo ufw allow 3000/tcp comment "ZeroClaw API server"

# Voice module
sudo ufw allow 8001/tcp comment "Voice module"

# Hermes Gateway
sudo ufw allow 8080/tcp comment "Hermes gateway"

# Prometheus metrics (internal only)
# sudo ufw allow from 127.0.0.1 to 127.0.0.1 port 9090 comment "Prometheus internal"

# Enable firewall
echo "y" | sudo ufw enable

# Display status
sudo ufw status verbose

echo "[✓] Firewall configured"
```

### 2.2 iptables Egress Control

Restrict outbound traffic to allowlisted domains (see network_allowlist.md):

```bash
#!/bin/bash
# setup-egress-rules.sh

# Apply network allowlist rules (from network_allowlist.md)
/usr/local/bin/zeroclaw-network-policy.sh

# Make rules persistent
sudo apt-get install -y iptables-persistent
sudo iptables-save | sudo tee /etc/iptables/rules.v4 > /dev/null
sudo ip6tables-save | sudo tee /etc/iptables/rules.v6 > /dev/null

# Enable persistence on reboot
sudo systemctl enable iptables-persistent
```

---

## 3. Docker Installation & Security

### 3.1 Install Docker & Docker Compose

```bash
#!/bin/bash
# install-docker.sh

set -e

echo "[*] Installing Docker and Docker Compose..."

# Add Docker's official repository
sudo apt-get remove -y docker docker.io containerd runc 2>/dev/null || true
sudo apt-get update
sudo apt-get install -y ca-certificates curl

sudo curl -fsSL https://download.docker.com/linux/ubuntu/gpg | sudo gpg --dearmor -o /usr/share/keyrings/docker-archive-keyring.gpg

echo "deb [arch=amd64 signed-by=/usr/share/keyrings/docker-archive-keyring.gpg] https://download.docker.com/linux/ubuntu $(lsb_release -cs) stable" | sudo tee /etc/apt/sources.list.d/docker.list > /dev/null

# Install Docker
sudo apt-get update
sudo apt-get install -y docker-ce docker-ce-cli containerd.io docker-compose-plugin

# Verify installation
docker --version
docker compose version

# Start Docker
sudo systemctl start docker
sudo systemctl enable docker

# Add zeroclaw user to docker group
sudo usermod -aG docker zeroclaw

echo "[✓] Docker installed successfully"
```

### 3.2 Docker Security Hardening

```bash
#!/bin/bash
# harden-docker.sh

set -e

echo "[*] Hardening Docker daemon..."

# Backup original config
sudo mkdir -p /etc/docker
sudo cp /etc/docker/daemon.json /etc/docker/daemon.json.backup 2>/dev/null || true

# Create hardened docker daemon config
sudo tee /etc/docker/daemon.json > /dev/null <<'EOF'
{
  "icc": false,
  "log-driver": "json-file",
  "log-opts": {
    "max-size": "10m",
    "max-file": "3"
  },
  "live-restore": true,
  "seccomp-profile": "/etc/docker/seccomp.json",
  "default-ulimits": {
    "nofile": {
      "Name": "nofile",
      "Hard": 65535,
      "Soft": 65535
    }
  },
  "userns-remap": "default",
  "storage-driver": "overlay2",
  "userland-proxy": false,
  "bridge": "",
  "metrics-addr": "127.0.0.1:9323"
}
EOF

# Create seccomp profile (restrictive)
sudo tee /etc/docker/seccomp.json > /dev/null <<'EOF'
{
  "defaultAction": "SCMP_ACT_ERRNO",
  "defaultErrnoRet": 1,
  "archMap": [
    {
      "architecture": "x86_64",
      "subArchitectures": ["x86", "x32"]
    }
  ],
  "syscalls": [
    {
      "names": ["accept", "accept4", "arch_prctl", "bind", "brk", "clone", "close", "connect", "dup", "dup2", "dup3", "epoll_create", "epoll_create1", "epoll_ctl", "epoll_wait", "epoll_pwait", "execve", "exit", "exit_group", "fcntl", "flock", "fstat", "fstatat", "futex", "futex2", "getcwd", "getegid", "getegid32", "geteuid", "geteuid32", "getgid", "getgid32", "getgroups", "getpeername", "getpid", "getppid", "getrandom", "getrlimit", "getrusage", "getsockname", "getsockopt", "gettimeofday", "getuid", "getuid32", "getxattr", "listxattr", "lseek", "lstat", "madvise", "memfd_create", "mmap", "mmap2", "mprotect", "mremap", "msync", "munmap", "name_to_handle_at", "nanosleep", "netlink", "open", "openat", "openat2", "pause", "perf_event_open", "perm", "pipe", "pipe2", "poll", "ppoll", "prctl", "pread64", "preadv", "preadv2", "prlimit64", "pselect6", "pwrite64", "pwritev", "pwritev2", "read", "readahead", "readlink", "readlinkat", "readv", "recvfrom", "recvmmsg", "recvmsg", "rename", "renameat", "renameat2", "restart_syscall", "rmdir", "rseq", "rt_sigaction", "rt_sigpending", "rt_sigprocmask", "rt_sigreturn", "rt_sigsuspend", "rt_sigtimedwait", "sched_getaffinity", "sched_getparam", "sched_getscheduler", "sched_yield", "seccomp", "seek", "select", "send", "sendfile", "sendfile64", "sendmmsg", "sendmsg", "sendto", "set_thread_area", "setfsgid", "setfsgid32", "setfsuid", "setfsuid32", "setgid", "setgid32", "setgroups", "setgroups32", "setitimer", "setpgid", "setpriority", "setregid", "setregid32", "setresgid", "setresgid32", "setresuid", "setresuid32", "setreuid", "setreuid32", "setrlimit", "setsid", "setsockopt", "settimeofday", "setuid", "setuid32", "setxattr", "sigaction", "sigaltstack", "signal", "signalfd", "signalfd4", "sigpending", "sigprocmask", "sigsuspend", "sigtimedwait", "sigwait", "sigwaitinfo", "socket", "socketpair", "splice", "stat", "statfs", "statx", "strace", "strlen", "symlink", "symlinkat", "sync", "sync_file_range", "sync_file_range2", "syncfs", "syslog", "tgkill", "time", "timerfd_create", "timerfd_gettime", "timerfd_settime", "times", "tkill", "truncate", "umask", "uname", "unlink", "unlinkat", "unshare", "usleep", "utime", "utimensat", "utimes", "vfork", "vhangup", "vmsplice", "wait3", "wait4", "waitid", "waitpid", "write", "writev"],
      "action": "SCMP_ACT_ALLOW"
    },
    {
      "names": ["ptrace", "process_vm_writev", "process_vm_readv"],
      "action": "SCMP_ACT_ALLOW",
      "includes": {
        "caps": ["SYS_PTRACE"]
      }
    },
    {
      "names": ["chown", "fchown", "fchownat", "lchown", "fchmod", "fchmodat", "chmod"],
      "action": "SCMP_ACT_ALLOW",
      "includes": {
        "caps": ["CHOWN", "DAC_OVERRIDE", "SETFCAP"]
      }
    }
  ]
}
EOF

# Reload Docker daemon
sudo systemctl daemon-reload
sudo systemctl restart docker

echo "[✓] Docker hardening applied"
```

### 3.3 Setup userns-remap for User Namespace Isolation

```bash
#!/bin/bash
# setup-userns-remap.sh

# Create default user for userns-remap
echo 'dockremap:165536:65536' | sudo tee /etc/subuid > /dev/null
echo 'dockremap:165536:65536' | sudo tee /etc/subgid > /dev/null

# Restart Docker to apply changes
sudo systemctl restart docker

echo "[✓] User namespace remapping configured"
```

---

## 4. ZeroClaw Deployment

### 4.1 Create Workspace & Configuration

```bash
#!/bin/bash
# setup-zeroclaw-workspace.sh

set -e

WORKSPACE="/zeroclaw-data/workspace"
CONFIG_DIR="/etc/zeroclaw"

echo "[*] Setting up ZeroClaw workspace..."

# Create directories
sudo mkdir -p "$WORKSPACE"/{primary,shared,delegates}
sudo mkdir -p "$CONFIG_DIR"
sudo mkdir -p /var/log/zeroclaw
sudo mkdir -p /var/zeroclaw

# Set permissions (zeroclaw user must own)
sudo chown -R zeroclaw:zeroclaw /zeroclaw-data
sudo chown -R zeroclaw:zeroclaw /var/log/zeroclaw
sudo chown -R zeroclaw:zeroclaw /var/zeroclaw

# Set permissions (restrictive)
sudo chmod 750 /zeroclaw-data
sudo chmod 750 /zeroclaw-data/workspace
sudo chmod 700 /var/log/zeroclaw
sudo chmod 700 /var/zeroclaw

echo "[✓] Workspace created"
```

### 4.2 Create Encrypted Secrets File

```bash
#!/bin/bash
# setup-secrets.sh

CONFIG_DIR="/etc/zeroclaw"

echo "[*] Setting up encrypted secrets..."

# Create .env template
sudo tee "$CONFIG_DIR/.env" > /dev/null <<'EOF'
# Alpaca Trading API
ALPACA_API_KEY=PK_your_api_key_here
ALPACA_SECRET_KEY=your_secret_key_here
ALPACA_LIVE_TRADING=false
ALPACA_BASE_URL=https://paper-api.alpaca.markets

# Voice & Notifications
ELEVENLABS_API_KEY=your_key_here
PUSHOVER_TOKEN=your_token_here
PUSHOVER_USER_KEY=your_user_key_here
TELEGRAM_BOT_TOKEN=your_bot_token_here

# LLM Services
OPENROUTER_API_KEY=sk-or-your_key_here
MEM0_API_KEY=your_mem0_key_here

# ByteRover
BYTEROVER_API_KEY=your_byterover_key_here

# Internal Settings
ZEROCLAW_ENV=production
ZEROCLAW_LOG_LEVEL=info
ZEROCLAW_WORKSPACE=/zeroclaw-data/workspace
EOF

# Encrypt the .env file
sudo openssl enc -aes-256-cbc \
    -in "$CONFIG_DIR/.env" \
    -out "$CONFIG_DIR/.env.enc" \
    -K $(openssl rand -hex 32) \
    -iv 00000000000000000000000000000000 \
    -md sha256

# Set restrictive permissions
sudo chmod 600 "$CONFIG_DIR/.env.enc"
sudo chmod 600 "$CONFIG_DIR/.env"

echo "[!] UPDATE: $CONFIG_DIR/.env with your actual API keys"
echo "[!] Then run: sudo /usr/local/bin/encrypt-env-secrets.sh"
echo "[!] Then restart: docker-compose restart zeroclaw"
```

### 4.3 Docker Compose File

```bash
#!/bin/bash
# Create /var/zeroclaw/docker-compose.yml

sudo mkdir -p /var/zeroclaw

sudo tee /var/zeroclaw/docker-compose.yml > /dev/null <<'EOF'
version: '3.9'

services:
  zeroclaw:
    image: zeroclaw:latest
    container_name: zeroclaw-agent
    restart: unless-stopped
    
    user: "1000:1000"  # Non-root user
    
    networks:
      - zeroclaw-internal
      - zeroclaw-services
    
    # Mount secrets (encrypted)
    volumes:
      - /etc/zeroclaw/.env.enc:/app/secrets/.env.enc:ro
      - /etc/zeroclaw/.env.key:/app/secrets/.env.key:ro
      - /zeroclaw-data/workspace:/zeroclaw-data/workspace:rw
      - /var/log/zeroclaw:/app/logs:rw
      - /var/zeroclaw/entrypoint-decrypt.sh:/entrypoint-decrypt.sh:ro
    
    # Security: drop all capabilities and add minimal ones
    cap_drop:
      - ALL
    cap_add:
      - NET_BIND_SERVICE
    
    # Security: no new privileges
    security_opt:
      - no-new-privileges:true
    
    # Read-only root filesystem
    read_only: true
    tmpfs:
      - /tmp:size=256m
      - /var/tmp:size=128m
      - /app/tmp:size=512m
    
    # Resource limits
    mem_limit: 1g
    memswap_limit: 2g
    cpus: '1.5'
    
    # DNS hardcoded
    dns:
      - 8.8.8.8
      - 8.8.4.4
      - 1.1.1.1
    dns_search: []
    
    # Network security
    sysctls:
      - net.ipv4.ip_forward=0
      - net.ipv4.conf.default.rp_filter=1
      - net.ipv4.icmp_echo_ignore_all=0
    
    # Environment (secrets loaded by entrypoint)
    environment:
      - ZEROCLAW_ENV=production
      - ZEROCLAW_LOG_LEVEL=info
      - ZEROCLAW_WORKSPACE=/zeroclaw-data/workspace
    
    # Health check
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:3000/health"]
      interval: 30s
      timeout: 10s
      retries: 3
      start_period: 40s
    
    # Logging
    logging:
      driver: json-file
      options:
        max-size: 10m
        max-file: 3
        labels: "service=zeroclaw"
    
    entrypoint: /entrypoint-decrypt.sh
    command: /app/zeroclaw

  # Hermes Gateway (optional, for sub-agent orchestration)
  hermes-gateway:
    image: hermes-gateway:latest
    container_name: hermes-gateway
    restart: unless-stopped
    
    networks:
      - zeroclaw-internal
      - zeroclaw-services
    
    environment:
      - ZEROCLAW_HOST=zeroclaw-agent:3000
      - LOG_LEVEL=info
    
    cap_drop:
      - ALL
    cap_add:
      - NET_BIND_SERVICE
    
    read_only: true
    tmpfs:
      - /tmp
    
    mem_limit: 512m
    cpus: '0.5'

networks:
  zeroclaw-internal:
    driver: bridge
    driver_opts:
      com.docker.network.bridge.name: br-zeroclaw
    ipam:
      config:
        - subnet: 172.25.0.0/16
          gateway: 172.25.0.1
  
  zeroclaw-services:
    driver: bridge
    driver_opts:
      com.docker.network.bridge.name: br-services
    ipam:
      config:
        - subnet: 172.26.0.0/16
          gateway: 172.26.0.1

volumes:
  zeroclaw-workspace:
    driver: local
    driver_opts:
      type: none
      o: bind
      device: /zeroclaw-data/workspace
EOF

sudo chown zeroclaw:zeroclaw /var/zeroclaw/docker-compose.yml
```

### 4.4 Deploy ZeroClaw Containers

```bash
#!/bin/bash
# deploy-zeroclaw.sh

set -e

cd /var/zeroclaw

echo "[*] Pulling ZeroClaw image..."
sudo docker pull zeroclaw:latest

echo "[*] Starting ZeroClaw containers..."
sudo docker-compose up -d

echo "[*] Waiting for containers to start..."
sleep 10

echo "[*] Container status:"
sudo docker-compose ps

echo "[*] Health check:"
sudo docker-compose exec zeroclaw curl http://localhost:3000/health || echo "Still starting..."

echo "[✓] Deployment complete"
```

---

## 5. Post-Deployment Configuration

### 5.1 Verify Security Settings

```bash
#!/bin/bash
# verify-security.sh

set -e

echo "[*] ZeroClaw Security Verification"
echo "==================================="

# Check firewall
echo -e "\n[*] Firewall status:"
sudo ufw status

# Check iptables egress rules
echo -e "\n[*] Network egress rules (sample):"
sudo iptables -L OUTPUT -nv | head -20

# Check Docker security
echo -e "\n[*] Docker daemon config:"
cat /etc/docker/daemon.json | jq '.icc, .userns-remap'

# Check ZeroClaw container
echo -e "\n[*] ZeroClaw container capabilities:"
sudo docker inspect zeroclaw-agent | jq '.[] | .HostConfig | {CapAdd, CapDrop, ReadonlyRootfs}'

# Check file permissions
echo -e "\n[*] Secret file permissions:"
ls -la /etc/zeroclaw/.env.enc

# Check logs
echo -e "\n[*] Recent logs:"
sudo tail -20 /var/log/zeroclaw/audit.log 2>/dev/null || echo "Logs not yet available"

echo -e "\n[✓] Security verification complete"
```

### 5.2 Setup Log Rotation

```bash
#!/bin/bash
# /etc/logrotate.d/zeroclaw

sudo tee /etc/logrotate.d/zeroclaw > /dev/null <<'EOF'
/var/log/zeroclaw/*.log {
    daily
    missingok
    rotate 14
    compress
    delaycompress
    notifempty
    create 0600 zeroclaw zeroclaw
    sharedscripts
    postrotate
        docker-compose -f /var/zeroclaw/docker-compose.yml \
            exec zeroclaw kill -HUP 1 > /dev/null 2>&1 || true
    endscript
}
EOF

sudo logrotate -f /etc/logrotate.d/zeroclaw
```

### 5.3 Setup Systemd Service (Optional)

```bash
#!/bin/bash
# /etc/systemd/system/zeroclaw.service

sudo tee /etc/systemd/system/zeroclaw.service > /dev/null <<'EOF'
[Unit]
Description=ZeroClaw AI Agent Runtime
After=docker.service docker.socket network-online.target
Requires=docker.service docker.socket
Wants=network-online.target

[Service]
Type=oneshot
RemainAfterExit=yes
User=zeroclaw
WorkingDirectory=/var/zeroclaw
ExecStart=/usr/bin/docker-compose up -d
ExecStop=/usr/bin/docker-compose down
Restart=always
RestartSec=10

[Install]
WantedBy=multi-user.target
EOF

# Enable and start
sudo systemctl daemon-reload
sudo systemctl enable zeroclaw
sudo systemctl start zeroclaw
sudo systemctl status zeroclaw
```

---

## 6. Monitoring & Maintenance

### 6.1 Setup Health Checks

```bash
#!/bin/bash
# zeroclaw-healthcheck.sh
# Runs periodically to verify ZeroClaw is healthy

CONTAINER="zeroclaw-agent"
LOG="/var/log/zeroclaw/health.log"

check_health() {
    # Check if container is running
    if ! docker inspect "$CONTAINER" 2>/dev/null | grep -q '"Running": true'; then
        echo "[!] $(date): Container not running!" >> "$LOG"
        return 1
    fi
    
    # Check API endpoint
    if ! docker exec "$CONTAINER" curl -f http://localhost:3000/health 2>/dev/null; then
        echo "[!] $(date): Health check failed" >> "$LOG"
        return 1
    fi
    
    # Check log for errors
    if docker logs "$CONTAINER" 2>/dev/null | grep -q "ERROR"; then
        echo "[!] $(date): Error found in logs" >> "$LOG"
    fi
    
    echo "[✓] $(date): Health check passed" >> "$LOG"
    return 0
}

check_health
```

### 6.2 Cron Jobs for Maintenance

```cron
# /etc/cron.d/zeroclaw-maintenance

# Daily health check
0 2 * * * zeroclaw /usr/local/bin/zeroclaw-healthcheck.sh

# Weekly log cleanup
0 3 * * 0 zeroclaw docker-compose -f /var/zeroclaw/docker-compose.yml logs --tail 0 > /dev/null

# Monthly secret rotation reminder (see secret_management.md)
0 9 1 1,4,7,10 * root /usr/local/bin/zeroclaw-secret-rotation-reminder.sh

# Quarterly security audit
0 0 1 1,4,7,10 * root /usr/local/bin/zeroclaw-security-audit.sh
```

---

## 7. Troubleshooting

### 7.1 Common Issues

```bash
# Container won't start
docker-compose logs zeroclaw

# Permission denied errors
sudo chmod 750 /zeroclaw-data/workspace
sudo chown zeroclaw:zeroclaw /zeroclaw-data -R

# Network connectivity issues
docker exec zeroclaw-agent ping 8.8.8.8
docker exec zeroclaw-agent curl https://api.alpaca.markets

# Secret decryption failed
sudo /usr/local/bin/encrypt-env-secrets.sh

# Out of disk space
docker system prune
docker image prune -a
```

### 7.2 Restart Procedures

```bash
#!/bin/bash
# graceful-restart.sh
# Safely restart ZeroClaw with zero downtime

cd /var/zeroclaw

echo "[*] Starting graceful restart..."

# Stop accepting new requests
# (Implementation depends on your router/LB)

# Wait for in-flight requests to complete
sleep 5

# Restart container
docker-compose restart zeroclaw

# Wait for health check
sleep 10

# Resume accepting requests
echo "[✓] Restart complete"
```

---

## 8. Full Deployment Script

```bash
#!/bin/bash
# deploy-all.sh
# Complete ZeroClaw deployment from fresh OS

set -e

echo "[*] ZeroClaw Full Deployment Starting..."

# Step 1: OS Setup
echo "[1/6] OS Hardening..."
bash setup-zeroclaw-user.sh
bash harden-ssh.sh
bash setup-firewall.sh
bash setup-egress-rules.sh

# Step 2: Docker Installation
echo "[2/6] Installing Docker..."
bash install-docker.sh
bash harden-docker.sh
bash setup-userns-remap.sh

# Step 3: Workspace & Secrets
echo "[3/6] Setting up workspace..."
bash setup-zeroclaw-workspace.sh
bash setup-secrets.sh

# Step 4: Docker Compose
echo "[4/6] Preparing Docker Compose..."
bash create-docker-compose.sh

# Step 5: Deployment
echo "[5/6] Deploying ZeroClaw..."
bash deploy-zeroclaw.sh

# Step 6: Verification
echo "[6/6] Verifying security..."
bash verify-security.sh

echo "[✓] DEPLOYMENT COMPLETE!"
echo ""
echo "Next steps:"
echo "1. Update secrets: sudo nano /etc/zeroclaw/.env"
echo "2. Encrypt: sudo bash /usr/local/bin/encrypt-env-secrets.sh"
echo "3. Restart: docker-compose restart zeroclaw"
echo "4. Verify: curl http://localhost:3000/health"
```

---

## 9. Deployment Checklist

- [ ] Fresh OS installed (Ubuntu 22.04 or Debian 12)
- [ ] System packages updated
- [ ] SSH hardened (no root login, key auth only)
- [ ] UFW firewall enabled
- [ ] iptables egress rules applied
- [ ] Docker installed and hardened
- [ ] ZeroClaw user created
- [ ] Workspace directories created
- [ ] Secrets encrypted
- [ ] Docker Compose file in place
- [ ] Containers deployed
- [ ] Health check passing
- [ ] Logs configured and rotating
- [ ] Monitoring/alerts set up
- [ ] Backup procedure documented

---

## 10. Security Hardening Summary

| Component | Hardening Applied |
|-----------|------------------|
| **SSH** | Key auth only, root disabled, timeout set |
| **Firewall** | UFW default deny, ports explicitly allowed |
| **Network** | iptables egress allowlist, DNS hardcoded |
| **Docker** | User namespaces, seccomp, read-only root, capability drops |
| **Container** | Non-root user, resource limits, tmpfs for temp files |
| **Secrets** | AES-256 encrypted, file-based, automatic cleanup |
| **Logs** | JSON format, rotation policy, audit trail |

---

## 11. Production Readiness Checklist

- [ ] All security hardening applied
- [ ] Secrets rotated and verified
- [ ] Monitoring alerts configured
- [ ] Backup/restore procedures tested
- [ ] Incident response plan documented
- [ ] Log aggregation setup (optional)
- [ ] Performance baseline established
- [ ] Disaster recovery plan in place

