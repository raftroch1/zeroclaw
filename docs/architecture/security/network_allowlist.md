# ZeroClaw Network Security — Allowlist & Implementation

**Version:** 1.0  
**Date:** 2025-04-14  
**Status:** Production Specification  
**Audience:** DevOps engineers, infrastructure teams

---

## Executive Summary

This document defines ZeroClaw's network security posture, including domain allowlists, implementation via iptables (recommended), Docker network policies, and upgrade paths to cloud-native networking solutions. The architecture enforces strict egress control: all outbound connections must explicitly match the allowlist, with default-deny for any undefined destinations.

**Key Principles:**
1. **Default deny**: No outbound traffic by default
2. **Explicit allowlisting**: Only approved domains/ports permitted
3. **Layered enforcement**: Both OS-level (iptables) and container-level (Docker) policies
4. **Audit logging**: Track all network decisions for compliance
5. **Rotation-ready**: Easy to update allowlists without container rebuild

---

## 1. Domain Allowlist Structure

### 1.1 Categorized Allowlist

```yaml
# Network allowlist structure
network_allowlist:
  
  # Trading & Market Data (Critical)
  trading:
    - domain: "api.alpaca.markets"
      ports: [443]
      protocol: "https"
      purpose: "Alpaca trading API (orders, positions, accounts)"
      rate_limit: "100 req/min"
      cert_validation: true
      
    - domain: "data.alpaca.markets"
      ports: [443]
      protocol: "https"
      purpose: "Alpaca market data feed"
      rate_limit: "unlimited"
      cert_validation: true
      
    - domain: "polygon.io"
      ports: [443]
      protocol: "https"
      purpose: "Market data aggregator (backup)"
      rate_limit: "50 req/min"
      cert_validation: true
      
    - domain: "api.live.coinbase.com"
      ports: [443]
      protocol: "https"
      purpose: "Coinbase real-time trading"
      rate_limit: "unlimited"
      cert_validation: true
  
  # External Tools & Services
  tools:
    - domain: "api.elevenlabs.io"
      ports: [443]
      protocol: "https"
      purpose: "Text-to-speech voice synthesis"
      rate_limit: "1000 req/hour"
      cert_validation: true
      
    - domain: "api.pushover.net"
      ports: [443, 80]
      protocol: "https"
      purpose: "Push notifications for alerts"
      rate_limit: "200 req/hour"
      cert_validation: true
      
    - domain: "api.telegram.org"
      ports: [443, 80]
      protocol: "https"
      purpose: "Telegram bot notifications"
      rate_limit: "unlimited"
      cert_validation: true
      
    - domain: "api.mem0.ai"
      ports: [443]
      protocol: "https"
      purpose: "Memory/context management service"
      rate_limit: "500 req/min"
      cert_validation: true
      
    - domain: "openrouter.ai"
      ports: [443]
      protocol: "https"
      purpose: "LLM API routing and inference"
      rate_limit: "1000 req/min"
      cert_validation: true
  
  # ByteRover Cloud Sync
  byterover:
    - domain: "api.byterover.dev"
      ports: [443]
      protocol: "https"
      purpose: "ByteRover cloud sync and metadata"
      rate_limit: "100 req/min"
      cert_validation: true
  
  # System & Monitoring (Internal)
  system:
    - domain: "prometheus:9090"
      ports: [9090]
      protocol: "http"
      purpose: "Prometheus metrics export"
      internal_only: true
      rate_limit: "unlimited"
      
    - domain: "grafana:3000"
      ports: [3000]
      protocol: "http"
      purpose: "Grafana monitoring dashboards"
      internal_only: true
      rate_limit: "unlimited"
  
  # DNS Resolution
  dns:
    - address: "8.8.8.8"
      ports: [53]
      protocol: "udp"
      purpose: "Google DNS primary"
      
    - address: "8.8.4.4"
      ports: [53]
      protocol: "udp"
      purpose: "Google DNS secondary"
      
    - address: "1.1.1.1"
      ports: [53]
      protocol: "udp"
      purpose: "Cloudflare DNS (backup)"
```

### 1.2 Blocked Domains (Explicit Deny)

```yaml
blocked_domains:
  - "localhost"      # Use Docker DNS instead
  - "127.0.0.1"      # Loopback (internal only)
  - "169.254.169.254" # AWS metadata service
  - "*.internal"      # Cloud provider metadata
  - "10.0.0.0/8"     # Private network scan
  - "172.16.0.0/12"  # Private network scan
  - "192.168.0.0/16" # Private network scan
```

---

## 2. iptables Implementation (Recommended)

### 2.1 Baseline iptables Rules

These rules should be applied on the Docker host or within the container depending on your security model. For ZeroClaw, we recommend applying at the host level for maximum control.

```bash
#!/bin/bash
# zeroclaw-network-policy.sh
# Sets up network allowlist using iptables

set -e

echo "[*] Configuring ZeroClaw network policies..."

# Define interfaces
DOCKER_BRIDGE="docker0"  # Or your custom bridge network
CONTAINER_IP="172.17.0.2"  # ZeroClaw container IP (adjust as needed)

# Enable IP forwarding
echo 1 > /proc/sys/net/ipv4/ip_forward

# ==================== INGRESS RULES ====================

# Default policy: ACCEPT (allow internal traffic)
iptables -P FORWARD ACCEPT
iptables -P INPUT ACCEPT

# Accept established/related connections
iptables -A INPUT -m state --state ESTABLISHED,RELATED -j ACCEPT
iptables -A FORWARD -m state --state ESTABLISHED,RELATED -j ACCEPT

# ==================== EGRESS RULES (OUTPUT) ====================

# Set default policy to DROP (deny all by default)
iptables -P OUTPUT DROP

# Allow loopback
iptables -A OUTPUT -o lo -j ACCEPT

# === DNS Rules ===
# Allow DNS to known resolvers (UDP port 53)
iptables -A OUTPUT -p udp -d 8.8.8.8 --dport 53 -j ACCEPT
iptables -A OUTPUT -p udp -d 8.8.4.4 --dport 53 -j ACCEPT
iptables -A OUTPUT -p udp -d 1.1.1.1 --dport 53 -j ACCEPT

# === HTTPS Traffic (port 443) ===
# API domains - Alpaca
iptables -A OUTPUT -p tcp -d api.alpaca.markets --dport 443 -j ACCEPT
iptables -A OUTPUT -p tcp -d data.alpaca.markets --dport 443 -j ACCEPT

# API domains - ElevenLabs
iptables -A OUTPUT -p tcp -d api.elevenlabs.io --dport 443 -j ACCEPT

# API domains - Pushover
iptables -A OUTPUT -p tcp -d api.pushover.net --dport 443 -j ACCEPT

# API domains - Telegram
iptables -A OUTPUT -p tcp -d api.telegram.org --dport 443 -j ACCEPT

# API domains - Mem0
iptables -A OUTPUT -p tcp -d api.mem0.ai --dport 443 -j ACCEPT

# API domains - OpenRouter
iptables -A OUTPUT -p tcp -d openrouter.ai --dport 443 -j ACCEPT

# API domains - ByteRover
iptables -A OUTPUT -p tcp -d api.byterover.dev --dport 443 -j ACCEPT

# API domains - Polygon.io
iptables -A OUTPUT -p tcp -d polygon.io --dport 443 -j ACCEPT

# API domains - Coinbase
iptables -A OUTPUT -p tcp -d api.live.coinbase.com --dport 443 -j ACCEPT

# === HTTP Traffic (port 80) - Limited ===
# Pushover allows HTTP fallback
iptables -A OUTPUT -p tcp -d api.pushover.net --dport 80 -j ACCEPT

# Telegram allows HTTP fallback
iptables -A OUTPUT -p tcp -d api.telegram.org --dport 80 -j ACCEPT

# === Internal Services (Docker network) ===
# Allow communication with other containers on same bridge
# Prometheus metrics
iptables -A OUTPUT -p tcp -d 172.17.0.3 --dport 9090 -j ACCEPT

# Grafana
iptables -A OUTPUT -p tcp -d 172.17.0.4 --dport 3000 -j ACCEPT

# Hermes Gateway (if on same host)
iptables -A OUTPUT -p tcp -d 172.17.0.5 --dport 8080 -j ACCEPT

# === Logging ===
# Log all denied packets (optional, for debugging)
iptables -A OUTPUT -m limit --limit 5/min -j LOG --log-prefix "iptables_deny: " --log-level 7

# Default drop
iptables -A OUTPUT -j DROP

echo "[✓] Network policies applied successfully"
echo "[*] Current OUTPUT rules:"
iptables -L OUTPUT -nv
```

### 2.2 Host-Level Container Network Control

For stricter control, apply rules per container IP:

```bash
#!/bin/bash
# zeroclaw-container-network.sh
# Network rules specific to ZeroClaw container

ZEROCLAW_IP="172.17.0.2"  # Adjust to your container IP

echo "[*] Configuring egress rules for ZeroClaw container ($ZEROCLAW_IP)..."

# Create custom chain for ZeroClaw
iptables -t filter -N ZEROCLAW_EGRESS 2>/dev/null || true

# Add all allowlisted domains to ZEROCLAW_EGRESS chain
# DNS
iptables -t filter -A ZEROCLAW_EGRESS -p udp -d 8.8.8.8 --dport 53 -j ACCEPT
iptables -t filter -A ZEROCLAW_EGRESS -p udp -d 8.8.4.4 --dport 53 -j ACCEPT
iptables -t filter -A ZEROCLAW_EGRESS -p udp -d 1.1.1.1 --dport 53 -j ACCEPT

# HTTPS allowed domains
for domain in "api.alpaca.markets" "data.alpaca.markets" "api.elevenlabs.io" \
              "api.pushover.net" "api.telegram.org" "api.mem0.ai" \
              "openrouter.ai" "api.byterover.dev" "polygon.io" \
              "api.live.coinbase.com"; do
    iptables -t filter -A ZEROCLAW_EGRESS -p tcp -d "$domain" --dport 443 -j ACCEPT
done

# HTTP fallback for specific domains
iptables -t filter -A ZEROCLAW_EGRESS -p tcp -d api.pushover.net --dport 80 -j ACCEPT
iptables -t filter -A ZEROCLAW_EGRESS -p tcp -d api.telegram.org --dport 80 -j ACCEPT

# Log and drop everything else
iptables -t filter -A ZEROCLAW_EGRESS -m limit --limit 5/min \
    -j LOG --log-prefix "ZEROCLAW_EGRESS_DENY: " --log-level 7
iptables -t filter -A ZEROCLAW_EGRESS -j DROP

# Jump to ZEROCLAW_EGRESS chain for traffic from ZeroClaw IP
iptables -t filter -A FORWARD -s $ZEROCLAW_IP -j ZEROCLAW_EGRESS
iptables -t filter -A OUTPUT -s $ZEROCLAW_IP -j ZEROCLAW_EGRESS

echo "[✓] ZeroClaw egress rules applied"
```

### 2.3 Persistence with iptables-persistent

```bash
# Install persistence utility
sudo apt-get install iptables-persistent

# Save current rules
sudo iptables-save > /etc/iptables/rules.v4
sudo ip6tables-save > /etc/iptables/rules.v6

# Rules will auto-restore on reboot
sudo systemctl enable iptables-persistent
sudo systemctl start iptables-persistent

# Verify rules persisted after reboot
sudo iptables -L OUTPUT -nv
```

---

## 3. Docker Network Policies

### 3.1 Docker Compose with Network Isolation

```yaml
# docker-compose.yml with network security
version: '3.9'

services:
  zeroclaw:
    image: zeroclaw:latest
    container_name: zeroclaw-agent
    
    networks:
      - zeroclaw-internal  # Isolated bridge network
      - zeroclaw-services  # Services network (restricted)
    
    # Network security
    cap_drop:
      - ALL
    cap_add:
      - NET_BIND_SERVICE      # Only if binding to ports
      - NET_RAW               # Dropped; see security section
    
    sysctls:
      - net.ipv4.ip_forward=0  # Prevent routing
      - net.ipv4.conf.default.rp_filter=1  # Reverse path filtering
    
    # DNS configuration - hardcoded resolvers
    dns:
      - 8.8.8.8
      - 8.8.4.4
      - 1.1.1.1
    
    # Disable DNS search domains (reduce attack surface)
    dns_opt:
      - ndots:0
    
    environment:
      # Network allowlist passed as env vars
      - ALLOWED_DOMAINS=api.alpaca.markets,data.alpaca.markets,api.elevenlabs.io
      - ALLOWED_PORTS=443,80
      - DEFAULT_DENY=true
    
    expose:
      - "3000"  # API server (internal only)
      - "8001"  # Voice module (internal only)

  # Hermes gateway - same network, separate container
  hermes-gateway:
    image: hermes-gateway:latest
    container_name: hermes-gateway
    networks:
      - zeroclaw-internal
    ports:
      - "8080:8080"  # Only exposed on specific interface
    environment:
      - ZEROCLAW_HOST=zeroclaw-agent:3000
      - ALLOWED_AGENTS=zeroclaw-agent

  # Prometheus for monitoring
  prometheus:
    image: prom/prometheus:latest
    container_name: prometheus
    networks:
      - zeroclaw-services
    ports:
      - "9090:9090"
    volumes:
      - ./prometheus.yml:/etc/prometheus/prometheus.yml:ro

  # Grafana for dashboards
  grafana:
    image: grafana/grafana:latest
    container_name: grafana
    networks:
      - zeroclaw-services
    ports:
      - "3000:3000"
    environment:
      - GF_SECURITY_ADMIN_PASSWORD=SecurePassword123!

networks:
  # ZeroClaw internal network - isolated from external traffic
  zeroclaw-internal:
    driver: bridge
    driver_opts:
      com.docker.network.bridge.name: br-zeroclaw
      com.docker.driver.mtu: 1500
    ipam:
      config:
        - subnet: 172.25.0.0/16
          gateway: 172.25.0.1
  
  # Services network for monitoring/logging
  zeroclaw-services:
    driver: bridge
    driver_opts:
      com.docker.network.bridge.name: br-services
    ipam:
      config:
        - subnet: 172.26.0.0/16
          gateway: 172.26.0.1
```

### 3.2 Advanced: OCI Runtime Hooks for Network Enforcement

```json
{
  "version": "1.0.0",
  "hooks": {
    "createRuntime": [
      {
        "path": "/usr/local/bin/zeroclaw-network-hook.sh",
        "args": ["zeroclaw-network-hook", "pre-start"],
        "env": [
          "CONTAINER_ID=${CONTAINER_ID}",
          "ALLOWED_DOMAINS=api.alpaca.markets,api.elevenlabs.io"
        ]
      }
    ]
  }
}
```

```bash
#!/bin/bash
# zeroclaw-network-hook.sh
# Runtime hook to enforce network policies before container start

CONTAINER_ID="$1"
ALLOWED_DOMAINS="${ALLOWED_DOMAINS:-}"

echo "[*] Network hook: Enforcing policies for $CONTAINER_ID"

# Get container IP
CONTAINER_IP=$(docker inspect -f '{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}' "$CONTAINER_ID")

if [ -z "$CONTAINER_IP" ]; then
    echo "[!] Failed to determine container IP"
    exit 1
fi

echo "[*] Container IP: $CONTAINER_IP, Allowed domains: $ALLOWED_DOMAINS"

# Apply iptables rules per container
# (Call the container-specific network script here)
/usr/local/bin/apply-zeroclaw-rules.sh "$CONTAINER_IP" "$ALLOWED_DOMAINS"

echo "[✓] Network policies enforced"
```

---

## 4. Network Policy as Code (Kubernetes Alternative)

For Kubernetes deployments, use NetworkPolicy resources:

```yaml
# zeroclaw-network-policy.yaml
apiVersion: networking.k8s.io/v1
kind: NetworkPolicy
metadata:
  name: zeroclaw-egress-policy
  namespace: zeroclaw
spec:
  podSelector:
    matchLabels:
      app: zeroclaw-agent
  policyTypes:
    - Egress
  egress:
    # Allow DNS
    - to:
        - namespaceSelector: {}
          podSelector:
            matchLabels:
              k8s-app: kube-dns
      ports:
        - protocol: UDP
          port: 53
    
    # Allow HTTPS to external APIs
    - to:
        - podSelector: {}
      ports:
        - protocol: TCP
          port: 443
      # Specific CIDR ranges for allowed domains
      # (Would require DNS resolver to CIDR mapping)
    
    # Deny all other egress
    # (Default action in NetworkPolicy)

---
# Service for ingress control
apiVersion: v1
kind: Service
metadata:
  name: zeroclaw-api
  namespace: zeroclaw
spec:
  type: ClusterIP
  selector:
    app: zeroclaw-agent
  ports:
    - port: 3000
      targetPort: 3000
      name: api
    - port: 8001
      targetPort: 8001
      name: voice
```

---

## 5. Firewall Integration (ufw)

For simpler deployments using `ufw` (Uncomplicated Firewall):

```bash
#!/bin/bash
# zeroclaw-ufw-rules.sh
# Configure UFW firewall for ZeroClaw

set -e

echo "[*] Configuring UFW firewall rules..."

# Enable UFW
sudo ufw default deny outgoing
sudo ufw default deny incoming
sudo ufw default allow routed

# Allow SSH (critical for management)
sudo ufw allow 22/tcp comment "SSH management"

# Allow Hermes Gateway (ingress)
sudo ufw allow 8080/tcp comment "Hermes gateway"

# Allow voice module (internal)
sudo ufw allow 8001/tcp comment "Voice module"

# Allow DNS (UDP 53)
sudo ufw allow out 8.8.8.8/32 port 53/udp comment "Google DNS"
sudo ufw allow out 8.8.4.4/32 port 53/udp comment "Google DNS secondary"
sudo ufw allow out 1.1.1.1/32 port 53/udp comment "Cloudflare DNS"

# Allow HTTPS (TCP 443) to specific domains
# Note: UFW doesn't support domain-based rules; use iptables for that
# Here we show the concept; actual implementation requires DNS to IP mapping

echo "[*] Setting up HTTPS allowlist..."

# Resolve domains to IPs and create rules
for domain in "api.alpaca.markets" "data.alpaca.markets" "api.elevenlabs.io" \
              "api.pushover.net" "api.telegram.org" "api.mem0.ai" \
              "openrouter.ai" "api.byterover.dev"; do
    
    # Get IPs for domain
    IPS=$(getent ahosts "$domain" 2>/dev/null | awk '{print $1}' | sort -u)
    
    for ip in $IPS; do
        sudo ufw allow out to "$ip" port 443/tcp comment "HTTPS: $domain"
        echo "[✓] Allowed $domain ($ip:443)"
    done
done

# Allow HTTP fallback for Pushover and Telegram
for domain in "api.pushover.net" "api.telegram.org"; do
    IPS=$(getent ahosts "$domain" 2>/dev/null | awk '{print $1}' | sort -u)
    for ip in $IPS; do
        sudo ufw allow out to "$ip" port 80/tcp comment "HTTP: $domain"
    done
done

# Enable UFW
sudo ufw --force enable

echo "[✓] UFW configuration complete"
sudo ufw status verbose
```

---

## 6. Egress Monitoring & Auditing

### 6.1 Log Denied Connections

```bash
#!/bin/bash
# monitor-zeroclaw-network.sh
# Monitor and audit ZeroClaw network activity

echo "[*] ZeroClaw Network Audit"
echo "================================"

# Denied egress connections
echo -e "\n[*] Recent denied egress (last 100 lines):"
sudo tail -100 /var/log/syslog | grep "iptables_deny" | tail -20

# Established connections by ZeroClaw
echo -e "\n[*] Established connections from ZeroClaw container:"
sudo netstat -tpn 2>/dev/null | grep zeroclaw

# DNS queries
echo -e "\n[*] DNS queries (tcpdump):"
sudo tcpdump -i docker0 -n "udp port 53" -c 10 2>/dev/null || echo "tcpdump not available"

# Connection count by domain
echo -e "\n[*] Connection attempts by domain (last hour):"
sudo grep "iptables_deny" /var/log/syslog | grep -oP 'DST=\K[^ ]*' | sort | uniq -c | sort -rn | head -10

# List current active connections
echo -e "\n[*] Current netstat summary:"
netstat -an | grep ESTABLISHED | wc -l
echo "Established connections found"
```

### 6.2 Prometheus Metrics for Network

```yaml
# prometheus-zeroclaw.yml
global:
  scrape_interval: 15s

scrape_configs:
  - job_name: 'zeroclaw-network'
    static_configs:
      - targets: ['localhost:9100']  # Node exporter
    metric_relabel_configs:
      # Capture network interface stats
      - source_labels: [__name__]
        regex: 'node_network.*'
        action: keep
      # Capture connection stats
      - source_labels: [__name__]
        regex: 'node_sockstat.*'
        action: keep

  - job_name: 'zeroclaw-app'
    static_configs:
      - targets: ['zeroclaw:3000/metrics']
    relabel_configs:
      # Add custom labels
      - source_labels: [__scheme__]
        replacement: 'https'
        target_label: __scheme__
```

---

## 7. Network Allowlist Update Procedure

### 7.1 Adding a New Domain

```bash
#!/bin/bash
# add-allowed-domain.sh
# Safely add a new domain to ZeroClaw allowlist

DOMAIN="$1"
PORT="${2:-443}"
PROTOCOL="${3:-https}"

if [ -z "$DOMAIN" ]; then
    echo "Usage: $0 <domain> [port] [protocol]"
    exit 1
fi

echo "[*] Adding domain to allowlist: $DOMAIN:$PORT ($PROTOCOL)"

# Resolve domain to IP
IP=$(getent ahosts "$DOMAIN" | head -1 | awk '{print $1}')

if [ -z "$IP" ]; then
    echo "[!] Failed to resolve domain: $DOMAIN"
    exit 1
fi

echo "[*] Domain resolves to: $IP"

# Add iptables rule (non-persistent, for testing)
iptables -A OUTPUT -p tcp -d "$IP" --dport "$PORT" -j ACCEPT -m comment --comment "Domain: $DOMAIN"

echo "[✓] Rule added to iptables"

# Update configuration file
cat >> /etc/zeroclaw/network-allowlist.txt << EOF
$DOMAIN:$PORT:$PROTOCOL
EOF

echo "[✓] Configuration updated"
echo "[!] Remember to run: sudo iptables-save > /etc/iptables/rules.v4"
```

### 7.2 Removing a Domain

```bash
#!/bin/bash
# remove-allowed-domain.sh

DOMAIN="$1"

if [ -z "$DOMAIN" ]; then
    echo "Usage: $0 <domain>"
    exit 1
fi

echo "[*] Removing domain from allowlist: $DOMAIN"

# Find and remove from config
sed -i "/^$DOMAIN/d" /etc/zeroclaw/network-allowlist.txt

# Get IP and remove iptables rules
IP=$(getent ahosts "$DOMAIN" | head -1 | awk '{print $1}')
if [ -n "$IP" ]; then
    iptables -D OUTPUT -p tcp -d "$IP" --dport 443 -j ACCEPT -m comment --comment "Domain: $DOMAIN" 2>/dev/null || true
fi

echo "[✓] Domain removed"
echo "[!] Remember to save: sudo iptables-save > /etc/iptables/rules.v4"
```

---

## 8. Network Allowlist Configuration Format

### 8.1 TOML Format (Recommended for Code)

```toml
# network-allowlist.toml

[dns]
primary = "8.8.8.8"
secondary = "8.8.4.4"
tertiary = "1.1.1.1"

[api.trading]
alpaca_trading = { domain = "api.alpaca.markets", port = 443, rate_limit = "100/min" }
alpaca_data = { domain = "data.alpaca.markets", port = 443, rate_limit = "unlimited" }
polygon = { domain = "polygon.io", port = 443, rate_limit = "50/min" }
coinbase = { domain = "api.live.coinbase.com", port = 443, rate_limit = "unlimited" }

[api.tools]
elevenlabs = { domain = "api.elevenlabs.io", port = 443, rate_limit = "1000/hour" }
pushover = { domain = "api.pushover.net", ports = [443, 80], rate_limit = "200/hour" }
telegram = { domain = "api.telegram.org", ports = [443, 80], rate_limit = "unlimited" }
mem0 = { domain = "api.mem0.ai", port = 443, rate_limit = "500/min" }
openrouter = { domain = "openrouter.ai", port = 443, rate_limit = "1000/min" }

[api.byterover]
cloud_sync = { domain = "api.byterover.dev", port = 443, rate_limit = "100/min" }

[blocked]
# Explicitly blocked for security
patterns = [
  "*.internal",
  "169.254.169.254",  # AWS metadata
  "169.254.169.253",  # Azure metadata
]
```

### 8.2 JSON Format (For Programmatic Access)

```json
{
  "version": "1.0",
  "timestamp": "2025-04-14T00:00:00Z",
  "dns": {
    "resolvers": ["8.8.8.8", "8.8.4.4", "1.1.1.1"]
  },
  "allowlist": [
    {
      "category": "trading",
      "entries": [
        {
          "domain": "api.alpaca.markets",
          "port": 443,
          "protocol": "https",
          "rate_limit": "100 req/min"
        },
        {
          "domain": "data.alpaca.markets",
          "port": 443,
          "protocol": "https"
        }
      ]
    },
    {
      "category": "tools",
      "entries": [
        {
          "domain": "api.elevenlabs.io",
          "port": 443,
          "protocol": "https"
        }
      ]
    }
  ],
  "blocked": [
    "*.internal",
    "169.254.169.254"
  ]
}
```

---

## 9. Testing Network Policies

### 9.1 Validation Script

```bash
#!/bin/bash
# test-network-policy.sh
# Verify ZeroClaw network policies are working

set -e

echo "[*] Testing ZeroClaw Network Policies"
echo "======================================"

# Test DNS resolution
echo -e "\n[*] Testing DNS resolution..."
nslookup api.alpaca.markets 8.8.8.8 > /dev/null && echo "[✓] DNS working" || echo "[!] DNS failed"

# Test allowed domains (should succeed)
echo -e "\n[*] Testing allowed domain access..."
curl -I --max-time 5 https://api.alpaca.markets 2>/dev/null | head -1 && echo "[✓] API access working" || echo "[!] API access failed"

# Test blocked domains (should fail)
echo -e "\n[*] Testing blocked domain rejection..."
if ! timeout 3 curl -I https://evil.com 2>/dev/null; then
    echo "[✓] Blocked domain correctly rejected"
else
    echo "[!] Blocked domain was not rejected!"
fi

# Verify iptables rules
echo -e "\n[*] Verifying iptables rules..."
if iptables -L OUTPUT -nv | grep -q "api.alpaca.markets"; then
    echo "[✓] iptables rules applied"
else
    echo "[!] iptables rules not found"
fi

# List current policies
echo -e "\n[*] Current network policies:"
iptables -L OUTPUT -nv | grep -E "(Chain|api\.|ACCEPT|DROP)" | head -20

echo -e "\n[✓] Network policy verification complete"
```

### 9.2 Inside-Container Testing

```bash
# Inside ZeroClaw container
echo "[*] Testing from inside container..."

# Test DNS
nslookup api.alpaca.markets

# Test HTTPS to allowed domain
curl -v https://api.alpaca.markets/v1/account

# Test rejection of blocked domain (will fail)
curl -v https://example.com
```

---

## 10. Upgrade Path: Cilium CNI (Kubernetes)

For cloud-native deployments, migrate to Cilium for eBPF-based network enforcement:

```yaml
# Install Cilium
helm repo add cilium https://helm.cilium.io
helm install cilium cilium/cilium \
  --namespace kube-system \
  --set kubeProxyReplacement=strict \
  --set l7Policies=true \
  --set policyEnforcementMode=always

# Cilium Policy for ZeroClaw
apiVersion: "cilium.io/v2"
kind: CiliumNetworkPolicy
metadata:
  name: "zeroclaw-egress"
  namespace: "zeroclaw"
spec:
  endpointSelector:
    matchLabels:
      app: zeroclaw-agent
  egress:
  - toEndpoints:
    - matchLabels:
        k8s-app: kube-dns
    toPorts:
    - ports:
      - port: "53"
        protocol: UDP
  - toFQDNs:
    - matchName: "api.alpaca.markets"
    - matchName: "data.alpaca.markets"
    - matchName: "api.elevenlabs.io"
    - matchName: "api.pushover.net"
    - matchName: "api.telegram.org"
    - matchName: "api.mem0.ai"
    - matchName: "openrouter.ai"
    - matchName: "api.byterover.dev"
    toPorts:
    - ports:
      - port: "443"
        protocol: TCP
      - port: "80"
        protocol: TCP
```

---

## 11. Summary & Checklist

### Implementation Checklist

- [ ] **iptables rules configured** at host level
- [ ] **Docker network isolation** enabled (bridge network)
- [ ] **DNS resolvers hardcoded** in container
- [ ] **Outbound policy set to DROP** by default
- [ ] **All allowlisted domains** have explicit ACCEPT rules
- [ ] **iptables rules persisted** (iptables-persistent)
- [ ] **Monitoring enabled** (syslog, netstat, tcpdump)
- [ ] **Audit logging** for network decisions
- [ ] **Testing procedures** validated
- [ ] **Domain update procedure** documented
- [ ] **Upgrade path** (Cilium/Kubernetes) planned

### Key Properties

✅ **Default deny**: All outbound traffic blocked unless explicitly allowed  
✅ **Explicit allowlisting**: Only approved domains/ports permitted  
✅ **Layered defense**: Both OS-level (iptables) and container-level (Docker)  
✅ **Audit trail**: All network decisions logged for compliance  
✅ **Zero trust**: No implicit trust in container behavior  

### Monitoring Dashboards

Metrics to expose in Prometheus/Grafana:
- Total bytes sent/received
- Denied connections by domain
- Connection latency to each API
- DNS resolution time
- Policy violation events

