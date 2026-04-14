#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "${ROOT_DIR}"

timestamp="$(date +%Y%m%d_%H%M%S)"
backup_root="${ROOT_DIR}/backups/deploy_${timestamp}"
mkdir -p "${backup_root}"

echo "[deploy] Backup directory: ${backup_root}"

if [[ ! -f .env ]]; then
  echo "[deploy] ERROR: .env not found. Copy .env.example to .env and set values." >&2
  exit 1
fi

# Ensure docker build context includes the production config at required path.
mkdir -p docker-config
cp config.toml docker-config/config.toml

echo "[deploy] Backing up critical local data..."
for item in soul.md config.toml workspace zeroclaw-data .brv; do
  if [[ -e "${item}" ]]; then
    cp -a "${item}" "${backup_root}/" || true
  fi
done

# Backup all SQLite/databases in project scope.
find . -maxdepth 4 -type f \( -name "*.db" -o -name "*.sqlite" -o -name "*.sqlite3" \) -print0 | \
  while IFS= read -r -d '' dbfile; do
    mkdir -p "${backup_root}/db/$(dirname "${dbfile}")"
    cp -a "${dbfile}" "${backup_root}/db/${dbfile}" || true
  done

backup_volume_if_exists() {
  local short_name="$1"
  local project_name
  project_name="${COMPOSE_PROJECT_NAME:-$(basename "${ROOT_DIR}")}" 

  local candidate1="${project_name}_${short_name}"
  local candidate2="${short_name}"
  local volume_name=""

  if docker volume inspect "${candidate1}" >/dev/null 2>&1; then
    volume_name="${candidate1}"
  elif docker volume inspect "${candidate2}" >/dev/null 2>&1; then
    volume_name="${candidate2}"
  fi

  if [[ -n "${volume_name}" ]]; then
    echo "[deploy] Backing up docker volume: ${volume_name}"
    docker run --rm -v "${volume_name}:/from:ro" -v "${backup_root}:/to" busybox \
      sh -c "tar -czf /to/${volume_name}.tgz -C /from ." || true
  fi
}

# Backup ByteRover knowledge + workspace/config volumes when present.
backup_volume_if_exists "zeroclaw-config"
backup_volume_if_exists "zeroclaw-workspace"
backup_volume_if_exists "zeroclaw-data"
backup_volume_if_exists "hermes-home"

echo "[deploy] Building containers..."
docker-compose build

echo "[deploy] Starting containers..."
docker-compose up -d

echo "[deploy] Running post-deploy verification..."
if ./verify.sh; then
  verify_status="PASS"
else
  verify_status="FAIL"
fi

echo ""
echo "========== DEPLOY STATUS =========="
echo "Backup:          ${backup_root}"
echo "Verification:    ${verify_status}"
echo ""
docker-compose ps
echo "==================================="

if [[ "${verify_status}" != "PASS" ]]; then
  echo "[deploy] Verification failed. Inspect logs with: docker-compose logs --tail=200" >&2
  exit 1
fi

echo "[deploy] Done. ZeroClaw deployment is up and verified."
