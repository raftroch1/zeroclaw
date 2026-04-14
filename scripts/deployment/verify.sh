#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "${ROOT_DIR}"

pass_count=0
fail_count=0

pass() {
  echo "[verify] PASS: $*"
  pass_count=$((pass_count + 1))
}

fail() {
  echo "[verify] FAIL: $*" >&2
  fail_count=$((fail_count + 1))
}

run_agent_shell() {
  local cmd="$1"
  local prompt
  prompt="Use the shell tool once and run this exact command: ${cmd}. Return only the tool output."
  docker-compose exec -T zeroclaw zeroclaw agent -m "${prompt}" 2>&1 || true
}

contains_any() {
  local haystack="$1"
  shift
  for needle in "$@"; do
    if [[ "${haystack,,}" == *"${needle,,}"* ]]; then
      return 0
    fi
  done
  return 1
}

echo "[verify] Checking container status..."
if docker-compose ps --services --filter "status=running" | grep -q "zeroclaw"; then
  pass "zeroclaw container is running"
else
  fail "zeroclaw container is not running"
fi

# 1) shell chaining should succeed
echo "[verify] Test 1: shell chaining"
out1="$(run_agent_shell 'echo "test" && echo "pass"')"
if contains_any "${out1}" "pass"; then
  pass "shell chaining accepted: echo \"test\" && echo \"pass\""
else
  fail "shell chaining did not produce expected output"
  echo "${out1}" >&2
fi

# 2) workspace redirect should succeed
echo "[verify] Test 2: safe redirect to workspace"
out2="$(run_agent_shell 'echo "test" > /zeroclaw-data/workspace/test.txt')"
if docker-compose exec -T zeroclaw test -f /zeroclaw-data/workspace/test.txt; then
  pass "safe redirect to /zeroclaw-data/workspace/test.txt succeeded"
else
  fail "safe redirect did not create /zeroclaw-data/workspace/test.txt"
  echo "${out2}" >&2
fi

# 3) blocked redirect outside workspace should fail
echo "[verify] Test 3: blocked redirect outside workspace"
out3="$(run_agent_shell 'echo "test" > /etc/test')"
if contains_any "${out3}" "blocked" "not allowed" "denied" "forbidden" "permission denied"; then
  pass "unsafe redirect outside workspace failed as expected"
else
  fail "unsafe redirect outside workspace was not clearly rejected"
  echo "${out3}" >&2
fi

# 4) pip availability
echo "[verify] Test 4: pip install requests"
out4="$(docker-compose exec -T zeroclaw bash -lc 'source /opt/venv/bin/activate && pip install requests' 2>&1 || true)"
if contains_any "${out4}" "Requirement already satisfied" "Successfully installed"; then
  pass "pip install requests succeeded"
else
  fail "pip install requests failed"
  echo "${out4}" >&2
fi

# 5) ByteRover status
echo "[verify] Test 5: ByteRover daemon status"
out5="$(docker-compose exec -T zeroclaw brv status 2>&1 || true)"
if contains_any "${out5}" "running" "healthy" "daemon"; then
  pass "brv status indicates daemon running"
else
  fail "brv status does not indicate daemon running"
  echo "${out5}" >&2
fi

# 6) Hermes health
echo "[verify] Test 6: Hermes /health"
out6="$(docker-compose exec -T zeroclaw curl -s -o /tmp/hermes_health.out -w '%{http_code}' http://hermes:8080/health 2>&1 || true)"
if [[ "${out6}" == "200" ]]; then
  pass "Hermes health endpoint returned 200"
else
  fail "Hermes health endpoint did not return 200 (got: ${out6})"
fi

# 7) Config production assertions
echo "[verify] Test 7: production config assertions"
config_dump="$(docker-compose exec -T zeroclaw bash -lc 'cat /zeroclaw-data/.zeroclaw/config.toml' 2>&1 || true)"
if contains_any "${config_dump}" 'default_provider = "openrouter"' 'level = "full"' 'auto_approve = ["shell"]'; then
  pass "config.toml contains production settings"
else
  fail "config.toml missing expected production settings"
  echo "${config_dump}" >&2
fi

echo ""
echo "[verify] Summary: ${pass_count} passed, ${fail_count} failed"
if [[ ${fail_count} -gt 0 ]]; then
  exit 1
fi
