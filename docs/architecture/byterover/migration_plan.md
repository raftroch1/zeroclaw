### ByteRover Migration Plan (SQLite + Mem0 Preserved)

#### Non-Negotiable Principles
1. **Do not replace existing memory systems.** SQLite and Mem0 remain active.
2. **ByteRover is additive.** It stores structured, curated knowledge only.
3. **Migrate selectively.** Bootstrap with high-value strategy knowledge, not full historical exhaust.

#### What Stays As-Is
- **SQLite**: all historical trades, execution logs, flags, config, session state.
- **Mem0**: all semantic lessons and narrative memory.
- Existing read/write paths continue unchanged; ByteRover is introduced as an additional route.

#### Migration Goals
- Initialize ByteRover project and tree scaffold.
- Seed known patterns/regime gates/time rules from current knowledge.
- Establish ongoing curation pipeline from future trades.

#### First-Run Initialization Procedure
1. Ensure `brv` CLI is installed and daemon can run.
2. In workspace root, initialize if missing:
   ```bash
   brv init --yes
   ```
3. Create initial node directories under `.brv/context-tree/alpaca-pattern-engine/...`.
4. Seed minimal canonical files per leaf node (short baseline guidance).
5. Validate retrieval:
   ```bash
   brv query "alpaca-pattern-engine What should I check before trading?"
   ```

#### Bootstrap from Existing Memories (Selective)
Focus domains to migrate first:
- trading patterns
- regime gates
- time-of-day setup constraints
- event risk controls
- risk parameter guardrails

Do not migrate:
- every trade narrative
- noisy one-off incidents
- redundant low-confidence notes

#### Extraction + Curation Procedure
1. **Extract from Mem0**
   - Query for high-signal themes (e.g., "ranging market losses", "open drive winners", "event-day mistakes").
2. **Normalize**
   - Convert free-text lessons to structured statements:
     - condition
     - action
     - expected outcome
     - confidence/evidence window
3. **Curate to ByteRover**
   - Use `brv curate` with explicit node-scoped context.
4. **Review & approve** (if review workflow enabled)
   - `brv review pending`
   - `brv review approve <taskId>`

#### Example Seed Script (Procedure)
```bash
#!/usr/bin/env bash
set -euo pipefail

# 1) Init
[ -f .brv/config.json ] || brv init --yes

# 2) Seed top-level context
brv curate "alpaca-pattern-engine/trading-patterns/breakout: Breakout entries require volume expansion and spread constraints."
brv curate "alpaca-pattern-engine/market-regimes/ranging: In ranging regimes, avoid continuation breakouts unless range expansion confirmed."
brv curate "alpaca-pattern-engine/time-setups/market-open: First 5 minutes prioritize liquidity confirmation over speed."
brv curate "alpaca-pattern-engine/safety/regime-gates: Disable trend-following setup in ranging+low-volume overlap."

# 3) Optional review
brv review pending || true
```

#### Step-by-Step Migration Workflow
1. **Prepare environment**
   - install CLI, enable daemon startup, verify `brv mcp`.
2. **Scaffold knowledge tree**
   - create domains + leaf nodes from `knowledge_tree_schema.md`.
3. **Seed canonical baseline**
   - add known pattern/risk/time/event rules.
4. **Backfill high-value Mem0 lessons**
   - select top lessons, normalize, curate into mapped nodes.
5. **Connect ZeroClaw routing**
   - pre-trade reads from ByteRover + SQLite state checks.
6. **Enable post-trade curation**
   - write raw to SQLite, lesson to Mem0, structured update to ByteRover.
7. **Run 2-week shadow validation**
   - compare decisions with and without ByteRover guidance.
8. **Promote to default structured layer**
   - keep graceful fallback if ByteRover unavailable.

#### Rollout Safety & Observability
- Log ByteRover availability at startup and each failed query/curation.
- Keep feature flag: `BYTEROVER_ENABLED=true|false`.
- On outage, continue with SQLite + Mem0 only.
- Weekly audit: compare curated ByteRover rules vs actual strategy behavior.
