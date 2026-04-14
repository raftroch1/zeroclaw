### Inter-Agent Communication Protocol

#### Purpose
Define deterministic communication between Lana and specialist agents through shared filesystem artifacts, with optional Hermes delegation bridge.

#### Communication channels
1. **Primary channel (required):** shared filesystem under `/workspace/shared/comm/`
2. **Secondary channel (optional):** Hermes API `POST /v1/delegate`
3. **Alert channel:** `/workspace/alerts/trades.log` (+ optional webhook)

#### Directory contract
- `/workspace/shared/comm/tasks/` — task envelopes
- `/workspace/shared/comm/results/` — successful outputs
- `/workspace/shared/comm/errors/` — structured failures
- `/workspace/shared/comm/audit/` — immutable event trail
- `/workspace/shared/comm/locks/` — lockfiles to avoid duplicate execution

#### Task delegation JSON format
```json
{
  "task_id": "uuid-v4",
  "workflow": "trading_analysis",
  "stage": "risk_check",
  "from_agent": "lana-orchestrator",
  "to_agent": "risk-manager",
  "created_at": "2026-04-14T12:00:00Z",
  "deadline_at": "2026-04-14T12:00:30Z",
  "priority": "high",
  "idempotency_key": "risk_check:trade_candidate:2026-04-14T12:00",
  "inputs": {
    "candidate_trade": {},
    "regime_decision": {},
    "account_snapshot": {}
  },
  "security": {
    "allowed_tools": ["read_file", "write_file"],
    "workspace_root": "/workspace/agents/risk-manager/",
    "credential_scope": "none"
  }
}
```

#### Result JSON format
```json
{
  "task_id": "uuid-v4",
  "status": "success",
  "agent": "risk-manager",
  "started_at": "2026-04-14T12:00:03Z",
  "completed_at": "2026-04-14T12:00:07Z",
  "duration_ms": 4011,
  "output": {
    "approved": false,
    "reasons": ["regime=ranging"],
    "limits_snapshot": {
      "trades_today": 2,
      "open_positions": 1,
      "daily_drawdown_pct": 1.1
    }
  },
  "artifacts": [
    "/workspace/shared/comm/results/uuid-v4.json"
  ]
}
```

#### Error JSON format
```json
{
  "task_id": "uuid-v4",
  "status": "error",
  "agent": "trade-executor",
  "error_code": "STOP_FLAG_ACTIVE",
  "retryable": false,
  "message": "Execution blocked due to /workspace/flags/STOP_TRADING",
  "context": {
    "stage": "execute_order",
    "workflow": "trade_execution"
  },
  "timestamp": "2026-04-14T12:03:11Z"
}
```

#### Hermes bridge payload
**Request**
```json
{
  "task": "Classify regime for SPY + QQQ",
  "tools": ["read_market_data", "compute_features"],
  "context": {
    "task_id": "uuid-v4",
    "workflow": "trading_analysis",
    "stage": "regime_analysis"
  }
}
```

**Response**
```json
{
  "status": "ok",
  "result": {
    "regime": "trending",
    "confidence": 0.81,
    "allow_trade": true
  }
}
```

#### Synthesis logic (orchestrator)
1. Gather required stage results in topological order.
2. Validate schema + freshness (`completed_at` within TTL).
3. Apply gate precedence:
   - `STOP_TRADING` flag
   - Regime gate (`ranging` => deny)
   - Hard risk limits
4. Select branch:
   - Execute branch if all gates pass
   - Skip branch otherwise
5. Emit consolidated decision artifact: `/workspace/shared/comm/results/<workflow_run_id>.json`

#### Error handling policy
- **Retryable errors** (timeouts, transient HTTP, lock contention): retry up to 2x with backoff.
- **Non-retryable errors** (policy violations, stop flag, schema violations): fail stage immediately.
- On stage failure with `on_error=abort`, terminate workflow and create incident audit record.
- On `on_error=continue`, mark degraded mode and continue with partial context.

#### Concurrency and locking
- One lockfile per `task_id`: `/workspace/shared/comm/locks/<task_id>.lock`
- Lock must contain owner agent and timestamp.
- Stale lock reclaim threshold: 120 seconds.

#### Audit requirements
Every stage emits an audit row with:
- `workflow_run_id`, `task_id`, `agent`, `status`, `duration_ms`, `resource_hint`, `decision_hash`
- Append-only storage in `/workspace/shared/comm/audit/events.jsonl`
