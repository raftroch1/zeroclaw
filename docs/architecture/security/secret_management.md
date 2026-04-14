# ZeroClaw Secret Management — API Keys & Credentials

**Version:** 1.0  
**Date:** 2025-04-14  
**Status:** Production Specification  
**Audience:** DevOps engineers, security teams, platform operators

---

## Executive Summary

This document specifies ZeroClaw's secret management architecture, covering how API keys, tokens, and credentials are stored, rotated, accessed, and audited. The current implementation uses encrypted environment variables mounted at container startup. This document also outlines the roadmap for future integration with enterprise secret management solutions (HashiCorp Vault, AWS Secrets Manager).

**Current Implementation:**
- Encrypted `.env` file on host, mounted as Docker secret or environment variable
- Secrets NOT stored in container image or Docker registry
- Manual rotation (admin updates `.env`, restarts container)
- Access controlled via Python SDK and Rust code

**Future State:**
- HashiCorp Vault integration for dynamic secret rotation
- AWS Secrets Manager for cloud deployments
- Automated secret rotation with zero downtime

---

## 1. Required Secrets Inventory

### 1.1 Trading API Credentials

```yaml
alpaca:
  # Alpaca Markets Trading API
  ALPACA_API_KEY:
    type: "API Key"
    scope: "Trading, Portfolio, Account"
    format: "PK_[A-Z0-9]{20}"
    expires: "Never (revoke in Alpaca dashboard)"
    rotation_frequency: "Quarterly or on compromise"
    used_by: ["zeroclaw-agent", "analysis-engine"]
    
  ALPACA_SECRET_KEY:
    type: "Secret Key"
    scope: "Authentication (paired with API key)"
    format: "Base64-encoded 256+ bits"
    expires: "Never (revoke with API key)"
    rotation_frequency: "Quarterly or on compromise"
    used_by: ["zeroclaw-agent"]
    
  ALPACA_LIVE_TRADING:
    type: "Boolean flag"
    scope: "Enable live trading (vs. paper trading)"
    format: "true|false"
    default: "false"  # Always start with paper trading
    
  ALPACA_BASE_URL:
    type: "Endpoint URL"
    scope: "API endpoint (paper or live)"
    format: "https://paper-api.alpaca.markets | https://api.alpaca.markets"
    default: "https://paper-api.alpaca.markets"
```

### 1.2 Voice & Notification API Credentials

```yaml
elevenlabs:
  ELEVENLABS_API_KEY:
    type: "API Key"
    scope: "Text-to-speech voice synthesis"
    format: "Alphanumeric, ~32 chars"
    expires: "Never (revoke in ElevenLabs dashboard)"
    rotation_frequency: "Quarterly"
    rate_limit: "1000 requests/hour (check dashboard)"
    used_by: ["zeroclaw-agent"]

pushover:
  PUSHOVER_TOKEN:
    type: "Application Token"
    scope: "Pushover notification service"
    format: "Alphanumeric, 30 chars"
    expires: "Never"
    rotation_frequency: "Yearly or on compromise"
    used_by: ["zeroclaw-agent"]
  
  PUSHOVER_USER_KEY:
    type: "User Key"
    scope: "Recipient for Pushover messages"
    format: "Alphanumeric, 30 chars"
    expires: "Never"
    rotation_frequency: "Yearly or on compromise"
    used_by: ["zeroclaw-agent"]

telegram:
  TELEGRAM_BOT_TOKEN:
    type: "Bot Token"
    scope: "Telegram bot command and notification"
    format: "DIGITS:ALPHANUM-chars (e.g., 123456:ABC-xyz)"
    expires: "Indefinite (revoke via @BotFather)"
    rotation_frequency: "On compromise only"
    used_by: ["zeroclaw-agent"]
    security_note: "Keep private! Do not commit to git."
```

### 1.3 LLM & AI Service Credentials

```yaml
openrouter:
  OPENROUTER_API_KEY:
    type: "API Key"
    scope: "LLM inference routing (Claude, GPT-4, etc.)"
    format: "sk-or-[A-Za-z0-9]{48}"
    expires: "Never (revoke in OpenRouter dashboard)"
    rotation_frequency: "Quarterly"
    rate_limit: "Check OpenRouter dashboard"
    used_by: ["zeroclaw-agent", "reasoning-engine"]
    billing: "Pay-as-you-go; monitor usage"

mem0:
  MEM0_API_KEY:
    type: "API Key"
    scope: "Memory/context management service"
    format: "Alphanumeric, typically 64+ chars"
    expires: "Never"
    rotation_frequency: "Quarterly"
    used_by: ["zeroclaw-agent"]
```

### 1.4 ByteRover Cloud Sync

```yaml
byterover:
  BYTEROVER_API_KEY:
    type: "API Key"
    scope: "ByteRover cloud synchronization"
    format: "Alphanumeric"
    expires: "Never"
    rotation_frequency: "Quarterly"
    used_by: ["zeroclaw-agent (subprocess)"]
```

### 1.5 Optional: AWS/Cloud Credentials (If Deployed on Cloud)

```yaml
aws:
  AWS_ACCESS_KEY_ID:
    type: "Access Key"
    scope: "AWS service authentication"
    format: "AKIA[0-9A-Z]{16}"
    expires: "Rotate every 90 days"
    used_by: ["zeroclaw-agent (if using AWS APIs)"]
    
  AWS_SECRET_ACCESS_KEY:
    type: "Secret Key"
    scope: "AWS authentication (paired with Access Key)"
    format: "Base64-encoded, 40 chars"
    expires: "Rotate every 90 days"
    used_by: ["zeroclaw-agent"]
    
  AWS_REGION:
    type: "Configuration"
    scope: "AWS region for services"
    format: "us-east-1, eu-west-1, etc."
    default: "us-east-1"
```

---

## 2. Secret Storage Specification

### 2.1 Host-Based Encrypted `.env` File

**Current production approach:**

```bash
# /etc/zeroclaw/.env (on host, encrypted)
# File permissions: 600 (owner read/write only)
# Location: HOST filesystem, NOT in container

# ============ Alpaca Trading API ============
ALPACA_API_KEY=PK_your_actual_api_key_here
ALPACA_SECRET_KEY=your_actual_secret_key_here
ALPACA_LIVE_TRADING=false
ALPACA_BASE_URL=https://paper-api.alpaca.markets

# ============ Voice & Notifications ============
ELEVENLABS_API_KEY=your_elevenlabs_key_here
PUSHOVER_TOKEN=your_pushover_token_here
PUSHOVER_USER_KEY=your_pushover_user_key_here
TELEGRAM_BOT_TOKEN=your_telegram_bot_token_here

# ============ LLM Services ============
OPENROUTER_API_KEY=sk-or-your_openrouter_key_here
MEM0_API_KEY=your_mem0_key_here

# ============ ByteRover ============
BYTEROVER_API_KEY=your_byterover_key_here

# ============ Internal Settings ============
ZEROCLAW_ENV=production
ZEROCLAW_LOG_LEVEL=info
ZEROCLAW_WORKSPACE=/zeroclaw-data/workspace
```

### 2.2 Host-Level Encryption (Recommended)

```bash
#!/bin/bash
# encrypt-env-secrets.sh
# Encrypt the .env file with openssl AES-256

set -e

ENV_FILE="/etc/zeroclaw/.env"
ENCRYPTED_FILE="/etc/zeroclaw/.env.enc"
KEY_FILE="/etc/zeroclaw/.env.key"

echo "[*] Encrypting ZeroClaw secrets with AES-256..."

# Generate 32-byte (256-bit) encryption key if it doesn't exist
if [ ! -f "$KEY_FILE" ]; then
    openssl rand -out "$KEY_FILE" 32
    chmod 600 "$KEY_FILE"
    echo "[✓] Generated new encryption key: $KEY_FILE"
fi

# Encrypt .env file
openssl enc -aes-256-cbc \
    -in "$ENV_FILE" \
    -out "$ENCRYPTED_FILE" \
    -K $(xxd -p -c 256 < "$KEY_FILE") \
    -iv 00000000000000000000000000000000 \
    -md sha256

chmod 600 "$ENCRYPTED_FILE"

# Verify encryption
openssl enc -aes-256-cbc -d \
    -in "$ENCRYPTED_FILE" \
    -K $(xxd -p -c 256 < "$KEY_FILE") \
    -iv 00000000000000000000000000000000 \
    -md sha256 | head -5

echo "[✓] Encryption successful"
echo "[!] Original .env should be deleted: rm $ENV_FILE"
```

```bash
#!/bin/bash
# decrypt-env-secrets.sh
# Decrypt .env file on container startup

ENCRYPTED_FILE="/etc/zeroclaw/.env.enc"
KEY_FILE="/etc/zeroclaw/.env.key"
TARGET_FILE="/tmp/.env"  # In-memory tmpfs

openssl enc -aes-256-cbc -d \
    -in "$ENCRYPTED_FILE" \
    -out "$TARGET_FILE" \
    -K $(xxd -p -c 256 < "$KEY_FILE") \
    -iv 00000000000000000000000000000000 \
    -md sha256

# Load into environment
export $(cat "$TARGET_FILE" | grep -v '^#' | xargs)

# Securely delete decrypted file from memory
shred -vfz -n 3 "$TARGET_FILE" 2>/dev/null || rm -f "$TARGET_FILE"

exec "$@"
```

### 2.3 Docker Compose with Secrets Mounting

```yaml
# docker-compose.yml with secret mounting
version: '3.9'

services:
  zeroclaw:
    image: zeroclaw:latest
    container_name: zeroclaw-agent
    
    # Mount encrypted secrets as volumes
    volumes:
      # Host .env.enc and key, mounted read-only
      - /etc/zeroclaw/.env.enc:/app/secrets/.env.enc:ro
      - /etc/zeroclaw/.env.key:/app/secrets/.env.key:ro
      
      # Startup script to decrypt
      - ./entrypoint-decrypt.sh:/entrypoint-decrypt.sh:ro
    
    # Use Docker secrets (preferred for Docker Swarm)
    # secrets:
    #   - zeroclaw_env
    #   - zeroclaw_encryption_key
    
    environment:
      # NOT setting secrets in env; they're mounted as files
      - ZEROCLAW_ENV=production
      - ZEROCLAW_LOG_LEVEL=info
    
    # Override entrypoint to decrypt secrets first
    entrypoint: /entrypoint-decrypt.sh
    command: /app/zeroclaw
    
    cap_drop:
      - ALL
    cap_add:
      - NET_BIND_SERVICE
    
    read_only: true
    tmpfs:
      - /tmp
      - /var/tmp
      - /app/secrets  # Decrypted secrets in memory

# Docker Swarm secrets (alternative approach)
secrets:
  zeroclaw_env:
    file: /etc/zeroclaw/.env.enc
  zeroclaw_encryption_key:
    file: /etc/zeroclaw/.env.key
```

---

## 3. Secret Rotation Procedures

### 3.1 Manual Rotation (Current Process)

```bash
#!/bin/bash
# rotate-secrets.sh
# Manually rotate API keys in ZeroClaw

set -e

SERVICE_NAME="${1:-zeroclaw}"
BACKUP_DIR="/var/backups/zeroclaw-secrets"
ENV_FILE="/etc/zeroclaw/.env"

echo "[*] Rotating secrets for $SERVICE_NAME..."

# Create backup
mkdir -p "$BACKUP_DIR"
cp "$ENV_FILE" "$BACKUP_DIR/.env.backup.$(date +%s)"
chmod 600 "$BACKUP_DIR"/.env.backup.*

echo "[!] Steps to rotate secrets:"
echo "1. Visit each service's dashboard:"
echo "   - Alpaca: https://app.alpaca.markets/settings/api-keys"
echo "   - ElevenLabs: https://elevenlabs.io/app/settings/api-keys"
echo "   - Pushover: https://pushover.net/apps"
echo "   - Telegram: @BotFather on Telegram"
echo "   - OpenRouter: https://openrouter.ai/settings"
echo ""
echo "2. Revoke old keys and generate new ones"
echo ""
echo "3. Update $ENV_FILE with new keys"
echo ""
echo "4. Encrypt: bash /usr/local/bin/encrypt-env-secrets.sh"
echo ""
echo "5. Restart container: docker-compose restart zeroclaw"
echo ""
echo "6. Verify by running: docker-compose logs zeroclaw | grep 'Connected'"

# Wait for manual update
read -p "[?] Press Enter after updating $ENV_FILE and encrypting..."

# Encrypt updated .env
/usr/local/bin/encrypt-env-secrets.sh

# Restart container
echo "[*] Restarting ZeroClaw container..."
docker-compose -f /var/zeroclaw/docker-compose.yml restart zeroclaw

# Wait for restart
sleep 5

# Verify connectivity
echo "[*] Verifying secret rotation..."
docker-compose -f /var/zeroclaw/docker-compose.yml logs zeroclaw | tail -20

echo "[✓] Secret rotation complete"
```

### 3.2 Scheduled Rotation (Cron Job)

```bash
#!/bin/bash
# /usr/local/bin/zeroclaw-secret-rotation-reminder.sh
# Sends reminder email for quarterly key rotation

EMAIL="ops@company.com"
SUBJECT="[ACTION REQUIRED] ZeroClaw Secret Rotation Due"

BODY="
ZeroClaw secret rotation is due.

Required rotations:
1. ALPACA_API_KEY + ALPACA_SECRET_KEY
2. ELEVENLABS_API_KEY
3. OPENROUTER_API_KEY
4. PUSHOVER_TOKEN
5. TELEGRAM_BOT_TOKEN

Steps:
1. Backup current /etc/zeroclaw/.env
2. Rotate keys in each service dashboard
3. Update .env with new keys
4. Encrypt: bash /usr/local/bin/encrypt-env-secrets.sh
5. Restart: docker-compose restart zeroclaw

Do NOT commit keys to git or Slack.
"

echo "$BODY" | mail -s "$SUBJECT" "$EMAIL"
```

```cron
# /etc/cron.d/zeroclaw-rotation
# Quarterly (every 3 months) secret rotation reminder

0 9 1 1,4,7,10 * /usr/local/bin/zeroclaw-secret-rotation-reminder.sh
```

### 3.3 Rotation Checklist (Per Service)

#### Alpaca API Keys

```markdown
## Alpaca Key Rotation

1. **Revoke old keys:**
   - Visit https://app.alpaca.markets/settings/api-keys
   - Click "Revoke" on the current API key
   - Wait 60 seconds for revocation to process

2. **Generate new keys:**
   - Click "Generate New Key"
   - Select appropriate scopes (Trading, Data, Account)
   - Copy API Key and Secret Key

3. **Update .env:**
   ```bash
   ALPACA_API_KEY=PK_xxxxx
   ALPACA_SECRET_KEY=xxxxx
   ```

4. **Test:**
   - Encrypt and restart
   - Verify trades can be placed (test on paper trading first)

5. **Document:**
   - Record rotation date
   - Note any API version changes
```

#### ElevenLabs API Key

```markdown
## ElevenLabs Key Rotation

1. **Generate new key:**
   - Visit https://elevenlabs.io/app/settings/api-keys
   - Click "Generate"
   - Copy the new key

2. **Update .env:**
   ```bash
   ELEVENLABS_API_KEY=xxxxx
   ```

3. **Test:**
   - Restart container
   - Test: `curl -X POST https://api.elevenlabs.io/v1/text-to-speech/test -H "Authorization: Bearer $ELEVENLABS_API_KEY"`

4. **Delete old key:**
   - Return to settings page
   - Delete the revoked key
```

#### Telegram Bot Token

```markdown
## Telegram Bot Token Rotation

1. **Revoke old token:**
   - Message @BotFather on Telegram
   - Type: `/revoke`
   - Select the bot
   - Confirm revocation

2. **Generate new token:**
   - Message @BotFather
   - Type: `/newtoken`
   - Select the bot
   - Copy new token

3. **Update .env:**
   ```bash
   TELEGRAM_BOT_TOKEN=xxxxx:xxxxx
   ```

4. **Test:**
   - Restart container
   - Verify bot responds to `/start`
```

---

## 4. Secret Access in Code

### 4.1 Python SDK Access Pattern

```python
# zeroclaw_sdk.py
import os
from typing import Dict, Optional

class ZeroClawSecretManager:
    """Manage API credentials safely."""
    
    def __init__(self):
        """Load secrets from environment (set by Docker at startup)."""
        self.alpaca_api_key = os.getenv('ALPACA_API_KEY')
        self.alpaca_secret_key = os.getenv('ALPACA_SECRET_KEY')
        self.elevenlabs_key = os.getenv('ELEVENLABS_API_KEY')
        self.pushover_token = os.getenv('PUSHOVER_TOKEN')
        self.pushover_user_key = os.getenv('PUSHOVER_USER_KEY')
        self.telegram_token = os.getenv('TELEGRAM_BOT_TOKEN')
        self.openrouter_key = os.getenv('OPENROUTER_API_KEY')
        self.mem0_key = os.getenv('MEM0_API_KEY')
        self.byterover_key = os.getenv('BYTEROVER_API_KEY')
        
        # Validate all required secrets are present
        self._validate_secrets()
    
    def _validate_secrets(self) -> None:
        """Ensure all required secrets are loaded."""
        required = {
            'ALPACA_API_KEY': self.alpaca_api_key,
            'ALPACA_SECRET_KEY': self.alpaca_secret_key,
            'ELEVENLABS_API_KEY': self.elevenlabs_key,
        }
        
        missing = [name for name, value in required.items() if not value]
        if missing:
            raise RuntimeError(f"Missing required secrets: {missing}")
    
    def get_alpaca_credentials(self) -> Dict[str, str]:
        """Get Alpaca trading credentials."""
        return {
            'api_key': self.alpaca_api_key,
            'secret_key': self.alpaca_secret_key,
            'paper': not os.getenv('ALPACA_LIVE_TRADING', 'false').lower() == 'true',
        }
    
    def get_elevenlabs_key(self) -> str:
        """Get ElevenLabs API key."""
        return self.elevenlabs_key
    
    def get_openrouter_key(self) -> str:
        """Get OpenRouter API key."""
        return self.openrouter_key
    
    # ... similar getters for other services ...
    
    def log_secret_access(self, service: str) -> None:
        """Log when a secret is accessed (for audit)."""
        import logging
        logger = logging.getLogger('security.secrets')
        logger.info(f"Secret accessed: {service}")  # Log only service name, never the value
```

### 4.2 Rust SDK Access Pattern

```rust
// src/secrets.rs
use std::env;
use std::sync::OnceLock;

pub struct SecretManager {
    alpaca_api_key: String,
    alpaca_secret_key: String,
    elevenlabs_key: String,
    openrouter_key: String,
    // ... other secrets
}

static SECRETS: OnceLock<SecretManager> = OnceLock::new();

impl SecretManager {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        Ok(SecretManager {
            alpaca_api_key: env::var("ALPACA_API_KEY")?,
            alpaca_secret_key: env::var("ALPACA_SECRET_KEY")?,
            elevenlabs_key: env::var("ELEVENLABS_API_KEY")?,
            openrouter_key: env::var("OPENROUTER_API_KEY")?,
        })
    }
    
    pub fn get() -> &'static SecretManager {
        SECRETS.get_or_init(|| {
            SecretManager::new().expect("Failed to load secrets")
        })
    }
    
    pub fn alpaca_api_key(&self) -> &str {
        &self.alpaca_api_key
    }
    
    pub fn elevenlabs_key(&self) -> &str {
        &self.elevenlabs_key
    }
    
    // ... other accessors
}

// Usage in code:
pub fn init_trading_client() -> Result<AlpacaClient, Box<dyn std::error::Error>> {
    let secrets = SecretManager::get();
    let client = AlpacaClient::new(
        secrets.alpaca_api_key(),
        secrets.alpaca_secret_key(),
    )?;
    
    audit_log("Secret accessed: ALPACA");  // Log only the service name
    Ok(client)
}
```

---

## 5. Secret Audit Logging

### 5.1 Audit Trail for Secret Access

```rust
// src/audit.rs
use chrono::Utc;
use std::fs::OpenOptions;
use std::io::Write;

pub struct SecretAuditLog {
    timestamp: String,
    event_type: String,  // accessed, rotated, denied
    service: String,     // alpaca, elevenlabs, etc
    agent_id: String,
    status: String,      // success, denied
    reason: String,      // if denied
}

impl SecretAuditLog {
    pub fn log(&self) -> Result<(), Box<dyn std::error::Error>> {
        let json = serde_json::json!({
            "timestamp": self.timestamp,
            "event_type": self.event_type,
            "service": self.service,
            "agent_id": self.agent_id,
            "status": self.status,
            "reason": self.reason,
        });
        
        // Write to audit log file (NOT to stdout)
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open("/var/log/zeroclaw/audit.log")?;
        
        writeln!(file, "{}", json.to_string())?;
        
        Ok(())
    }
}

// Usage:
pub fn access_secret(service: &str, agent_id: &str) -> Result<String, String> {
    // Log access attempt
    let log = SecretAuditLog {
        timestamp: Utc::now().to_rfc3339(),
        event_type: "accessed".into(),
        service: service.to_string(),
        agent_id: agent_id.to_string(),
        status: "success".into(),
        reason: String::new(),
    };
    
    let _ = log.log();
    
    // Return secret
    Ok(get_secret_from_env(service))
}
```

### 5.2 Monitoring Secret Rotation

```bash
#!/bin/bash
# monitor-secret-rotation.sh
# Track when secrets were last rotated

AUDIT_LOG="/var/log/zeroclaw/audit.log"

echo "[*] Secret Rotation Status"
echo "============================"

for service in alpaca elevenlabs telegram openrouter pushover byterover; do
    last_rotation=$(grep "event_type.*rotated" "$AUDIT_LOG" | \
                   grep "service.*$service" | \
                   tail -1 | \
                   jq -r .timestamp)
    
    if [ -z "$last_rotation" ]; then
        last_rotation="Never"
    fi
    
    echo "[$service] Last rotation: $last_rotation"
done

# Calculate days since last rotation
echo ""
echo "[*] Secrets requiring rotation (>90 days):"
grep "event_type.*rotated" "$AUDIT_LOG" | \
    jq -r '.timestamp' | \
    while read timestamp; do
        days_ago=$(( ($(date +%s) - $(date -d "$timestamp" +%s)) / 86400 ))
        if [ "$days_ago" -gt 90 ]; then
            echo "  - $timestamp ($days_ago days ago)"
        fi
    done
```

---

## 6. Future: Vault Integration

### 6.1 HashiCorp Vault Setup

```bash
#!/bin/bash
# setup-vault-integration.sh
# Prepare ZeroClaw for Vault integration

VAULT_ADDR="https://vault.internal:8200"
VAULT_TOKEN_ROLE="zeroclaw"

echo "[*] Setting up HashiCorp Vault integration..."

# 1. Authenticate ZeroClaw to Vault
# (This would use Kubernetes auth, approle, or JWT)

# 2. Configure Vault secret engine
vault secrets enable -path=zeroclaw kv-v2

# 3. Store secrets in Vault
vault kv put zeroclaw/trading \
    alpaca_api_key="PK_xxx" \
    alpaca_secret_key="xxx"

vault kv put zeroclaw/services \
    elevenlabs_key="xxx" \
    openrouter_key="xxx" \
    pushover_token="xxx" \
    telegram_token="xxx"

# 4. Create policy for ZeroClaw
vault policy write zeroclaw - <<EOF
path "zeroclaw/data/*" {
  capabilities = ["read", "list"]
}

path "zeroclaw/metadata/*" {
  capabilities = ["list"]
}
EOF

# 5. Configure dynamic credentials (future)
# vault write zeroclaw/config/connection ...
```

### 6.2 Rust Client for Vault

```rust
// In Cargo.toml:
// [dependencies]
// vaultrs = "0.7"

use vaultrs::client::Client;
use std::env;

pub async fn load_secrets_from_vault() -> Result<SecretManager, Box<dyn std::error::Error>> {
    let vault_addr = env::var("VAULT_ADDR")
        .unwrap_or_else(|_| "https://vault.internal:8200".to_string());
    
    let vault_token = env::var("VAULT_TOKEN")?;
    
    let client = Client::new(
        Some(vault_addr.as_str()),
        Some(&vault_token),
        None,
    )?;
    
    // Read trading credentials
    let trading_secrets: serde_json::Value = client
        .read("zeroclaw/data/trading")
        .await?;
    
    // Read service credentials
    let service_secrets: serde_json::Value = client
        .read("zeroclaw/data/services")
        .await?;
    
    Ok(SecretManager {
        alpaca_api_key: trading_secrets["data"]["data"]["alpaca_api_key"]
            .as_str()
            .unwrap_or("")
            .to_string(),
        // ... other fields
    })
}

// Dynamic secret rotation (future)
pub async fn watch_vault_secrets() {
    // Periodically refresh secrets from Vault
    loop {
        tokio::time::sleep(Duration::from_secs(3600)).await;  // Hourly
        
        if let Ok(secrets) = load_secrets_from_vault().await {
            // Update in-memory secrets
            SECRETS.take();  // Clear cache
            // Secrets will be reloaded on next access
        }
    }
}
```

---

## 7. Secret Scanning & Prevention

### 7.1 Git Pre-commit Hook

```bash
#!/bin/bash
# .git/hooks/pre-commit
# Prevent committing secrets to git

set -e

echo "[*] Scanning for secrets in staged files..."

# Patterns that look like secrets
PATTERNS=(
    "ALPACA_API_KEY"
    "ALPACA_SECRET_KEY"
    "ELEVENLABS_API_KEY"
    "OPENROUTER_API_KEY"
    "TELEGRAM_BOT_TOKEN"
    "sk-or-"  # OpenRouter key pattern
    "sk-ant-" # Anthropic key pattern
    "PK_"     # Alpaca key pattern
)

FOUND_SECRETS=0

for pattern in "${PATTERNS[@]}"; do
    if git diff --cached | grep -q "$pattern"; then
        echo "[!] WARNING: Possible secret in staged changes: $pattern"
        FOUND_SECRETS=$((FOUND_SECRETS + 1))
    fi
done

if [ $FOUND_SECRETS -gt 0 ]; then
    echo "[!] ERROR: $FOUND_SECRETS potential secrets found!"
    echo "[!] Do NOT commit secrets. Use environment variables or .env file."
    exit 1
fi

echo "[✓] No secrets detected"
exit 0
```

### 7.2 Docker Image Scanning

```bash
#!/bin/bash
# scan-docker-image.sh
# Verify secrets are not baked into Docker image

IMAGE="zeroclaw:latest"

echo "[*] Scanning Docker image for secrets: $IMAGE"

# Extract and scan all file contents
docker image inspect "$IMAGE" > /tmp/image-inspect.json

# Check for secret patterns in image config/layers
PATTERNS=(
    "ALPACA_"
    "ELEVENLABS_"
    "OPENROUTER_"
    "TELEGRAM_"
    "PK_"
    "sk-"
)

FOUND=0
for pattern in "${PATTERNS[@]}"; do
    if docker run --rm "$IMAGE" grep -r "$pattern" / 2>/dev/null | grep -q "var"; then
        echo "[!] Found potential secret pattern: $pattern"
        FOUND=$((FOUND + 1))
    fi
done

if [ $FOUND -gt 0 ]; then
    echo "[!] ERROR: Secrets detected in image!"
    exit 1
else
    echo "[✓] No hardcoded secrets detected"
fi
```

---

## 8. Implementation Checklist

- [ ] Create encrypted `.env` file on host (`/etc/zeroclaw/.env.enc`)
- [ ] Implement Docker secret mounting
- [ ] Add secret access logging to audit trail
- [ ] Test secret rotation procedure for each service
- [ ] Document secret rotation checklist per service
- [ ] Set up quarterly rotation reminders (cron)
- [ ] Configure git pre-commit hook to prevent secret commits
- [ ] Add Docker image scanning to CI/CD pipeline
- [ ] Plan Vault integration for future
- [ ] Test secret access from both Python and Rust code
- [ ] Document secret management in runbook

---

## 9. Summary: Secret Management Principles

✅ **Secrets never in code**: All keys loaded from environment  
✅ **Encrypted at rest**: `.env` encrypted on host  
✅ **Limited in-memory**: Secrets only in OnceLock<> or once-loaded  
✅ **Audited on access**: Every secret access logged (service name only)  
✅ **Rotated regularly**: Quarterly rotation schedule + on-demand  
✅ **Compartmentalized**: Each agent can't access parent's secrets  
✅ **Vault-ready**: Design ready for Vault integration  
✅ **Clean on restart**: Secrets cleared from memory on process restart  

