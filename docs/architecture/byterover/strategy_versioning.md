### ByteRover Strategy Versioning for Trading Experiments

#### Why Use ByteRover VC
Trading strategy evolution should be testable and reversible. ByteRover VC provides branch/commit/merge semantics for structured strategy knowledge.

#### Command Mapping to Trading Actions
- **`brv vc branch <name>`** → "Try new strategy variant"
  - Example: create `vix-filter` variant.
- **`brv vc commit -m "..."`** → "Checkpoint current strategy state"
  - Snapshot rules, regime gates, and assumptions at a point in time.
- **`brv vc merge <source> <target>`** → "Adopt successful variant"
  - Promote tested variant into baseline.

#### Standard Experiment Workflow
1. Branch from baseline.
2. Curate hypothesis and changed rules into variant node.
3. Run live/sim period for fixed window (e.g., 30 trading days).
4. Compare objective metrics vs baseline.
5. Merge if success criteria met; otherwise discard/keep archived.

#### Example: VIX > 25 Filter Test
```bash
# 1) Create variant branch
brv vc branch vix-filter

# 2) Document variant hypothesis
brv curate "alpaca-pattern-engine/strategy-variants/vix-filter: Hypothesis: avoid trades when VIX > 25 to reduce left-tail losses."

# 3) Checkpoint initial variant state
brv vc commit -m "Initialize vix-filter variant with VIX gate threshold 25"

# ... run experiment for 30 trading days ...

# 4) Curate result summary
brv curate "alpaca-pattern-engine/performance/strategy-metrics: vix-filter 30d results: win_rate=0.57, expectancy=+0.11R, max_dd=-3.2R"

# 5A) If successful, merge into baseline
brv vc merge vix-filter baseline
brv vc commit -m "Merge vix-filter into baseline after 30d outperformance"

# 5B) If unsuccessful, keep/discard branch (team policy)
# (No merge; optionally tag/archive branch in team notes.)
```

#### Decision Criteria (Concrete)
Adopt variant only if all criteria pass over agreed sample window:
- Win rate increase >= **+3 percentage points** OR expectancy increase >= **+0.08R**
- Max drawdown not worse than baseline by more than **0.5R**
- Trade count above minimum statistical floor (e.g., **>= 40 trades**)
- No new risk-policy violations introduced

#### Rollback Playbook
If merged strategy underperforms post-adoption:
1. Compare baseline history vs merged branch metrics.
2. Identify change-set causing degradation.
3. Revert by promoting prior stable branch/state.
4. Curate root-cause note into performance + safety nodes.

Example rollback-oriented commands:
```bash
# Inspect branch state (team may also track external diff artifacts)
brv query "alpaca-pattern-engine/strategy-variants baseline vs vix-filter key rule differences"

# Re-branch from known stable baseline snapshot
brv vc branch baseline-recovery
brv curate "alpaca-pattern-engine/strategy-variants/baseline-recovery: rollback to pre-vix-merge due to degraded expectancy."
brv vc commit -m "Rollback to stable baseline configuration"
```

#### Governance Recommendations
- One hypothesis per branch.
- Predefine metric gates before test starts.
- Require end-of-window review note before any merge.
- Keep risk and safety node updates in same commit window as strategy rule updates.
