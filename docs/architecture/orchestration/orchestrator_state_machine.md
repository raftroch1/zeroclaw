### Lana Orchestrator State Machine

#### Objective
Define operating modes (Solo, Delegate, Swarm), transitions, health checks, and failure recovery for autonomous trading orchestration.

#### Core modes
1. **Solo**
   - Lana executes pipeline directly, minimal delegation.
   - Best for low-latency simple flows and diagnostics.

2. **Delegate**
   - Lana delegates specific stages to in-process specialists.
   - Constraint: max 3 concurrent delegates, depth <= 2.

3. **Swarm**
   - Lana coordinates mixed workers (in-process + Hermes sub-agents).
   - Used for parallel analysis, heavier context, or recovery fallback.

#### States
- `INIT`
- `READY`
- `RUNNING_SOLO`
- `RUNNING_DELEGATE`
- `RUNNING_SWARM`
- `DEGRADED`
- `PAUSED_STOP_FLAG`
- `EMERGENCY_HALTED`
- `RECOVERING`
- `SHUTDOWN`

#### Transition rules
- `INIT -> READY`: config + workspace + tool checks pass.
- `READY -> RUNNING_SOLO`: requested mode is solo.
- `READY -> RUNNING_DELEGATE`: delegation allowed and health checks green.
- `READY -> RUNNING_SWARM`: workload requests parallel fanout and Hermes reachable.
- `RUNNING_* -> PAUSED_STOP_FLAG`: `/workspace/flags/STOP_TRADING` detected.
- `PAUSED_STOP_FLAG -> READY`: flag removed + manual resume note recorded.
- `RUNNING_* -> DEGRADED`: repeated transient failures exceed threshold.
- `DEGRADED -> RECOVERING`: backoff + subsystem restarts initiated.
- `RECOVERING -> READY`: health checks pass.
- `RUNNING_* -> EMERGENCY_HALTED`: emergency stop protocol fired (manual or policy trigger).
- `EMERGENCY_HALTED -> READY`: explicit operator clearance + post-incident checklist complete.
- `* -> SHUTDOWN`: orchestrator termination requested.

#### Health checks
- **Filesystem:** required dirs writable/readable under `/workspace/`
- **Safety files:** `safety_config.toml` parseable; STOP flag check responsive
- **Broker:** Alpaca paper endpoint latency + auth sanity
- **Hermes:** `/v1/delegate` liveness probe when in swarm mode
- **Memory layers:** SQLite reachable, Mem0 endpoint reachable, `brv query` sanity command returns

#### Failure recovery policy
1. Classify failure: transient, policy, dependency, data-integrity.
2. For transient: retry with capped exponential backoff.
3. For dependency outage:
   - Swarm -> Delegate fallback
   - Delegate -> Solo fallback
4. For policy breach: hard block and record reason.
5. For integrity breach: quarantine artifact and require re-computation.

#### Degradation matrix
- Hermes down: disable swarm; continue in delegate/solo.
- ByteRover down: continue with SQLite + Mem0, mark `knowledge_degraded=true`.
- Alpaca trading API down: disable execution; keep analysis + reporting active.
- Notification sink down: queue alerts locally and retry flush.

#### Minimal pseudocode
```text
while running:
  load_state()
  if stop_flag_exists(): transition(PAUSED_STOP_FLAG)
  if state in RUNNING and emergency_condition(): transition(EMERGENCY_HALTED)
  execute_next_work_item_by_mode()
  run_health_checks()
  if failures >= threshold: transition(DEGRADED)
  if state == DEGRADED: attempt_recovery()
```
