### Autonomous Loop Cron Definitions

#### Assumptions
- Timezone: **UTC**
- Market reference: US equities regular session (13:30-20:00 UTC during DST)
- Crontab runs inside Lana container
- Each cron command should call orchestrator entrypoint with workflow TOML path

#### Recommended crontab entries
```cron
# 1) Pre-market scan (weekdays) at 12:45 UTC
45 12 * * 1-5 /usr/local/bin/lana-orchestrator run /workspace/shared/orchestration/workflow_templates/trading_analysis.toml --mode premarket-scan >> /workspace/logs/cron/premarket.log 2>&1

# 2) Market-hours polling loop (weekdays): every 5 minutes from 13:30 to 19:55 UTC
*/5 13-19 * * 1-5 /usr/local/bin/lana-orchestrator run /workspace/shared/orchestration/workflow_templates/trading_analysis.toml --mode market-poll >> /workspace/logs/cron/market_poll.log 2>&1

# 3) Top-of-hour closeout check during last hour
0 20 * * 1-5 /usr/local/bin/lana-orchestrator run /workspace/shared/orchestration/workflow_templates/trade_execution.toml --mode closeout-check >> /workspace/logs/cron/closeout.log 2>&1

# 4) After-hours daily summary (weekdays)
15 21 * * 1-5 /usr/local/bin/lana-orchestrator run /workspace/shared/orchestration/workflow_templates/daily_summary.toml >> /workspace/logs/cron/daily_summary.log 2>&1

# 5) Weekend deep knowledge curation (Saturday)
0 14 * * 6 /usr/local/bin/lana-orchestrator run /workspace/shared/orchestration/workflow_templates/knowledge_curation.toml --mode weekend-batch >> /workspace/logs/cron/weekend_curation.log 2>&1

# 6) Safety heartbeat every minute
* * * * * /usr/local/bin/lana-orchestrator healthcheck --safety-config /workspace/shared/orchestration/safety_config.toml >> /workspace/logs/cron/heartbeat.log 2>&1
```

#### Scheduling behavior notes
- If `/workspace/flags/STOP_TRADING` exists, trading workflows run in **no-execution** mode and only generate diagnostics.
- Add random jitter (0-15s) inside orchestrator to reduce contention on external APIs.
- Lock by workflow name to prevent overlapping runs (`/workspace/shared/comm/locks/workflow-<name>.lock`).

#### Operational checklist
- Ensure `/workspace/logs/cron/` exists.
- Rotate logs daily or by size.
- Alert on 3 consecutive non-zero exits for any trading workflow.
