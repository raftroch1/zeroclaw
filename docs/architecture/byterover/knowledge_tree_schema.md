### Alpaca Pattern Engine ByteRover Knowledge Tree Schema

#### Scope
Initial tree with 29 nodes (including parent domains + leaf topics). This is a starting scaffold; Lana can expand organically.

```text
alpaca-pattern-engine/
├── trading-patterns/
│   ├── double-top/
│   ├── head-shoulders/
│   ├── breakout/
│   ├── pullback-continuation/
│   ├── failed-breakout/
│   └── gap-and-go/
├── market-regimes/
│   ├── trending/
│   ├── ranging/
│   ├── volatile/
│   ├── low-volume/
│   └── risk-on-risk-off/
├── time-setups/
│   ├── pre-market/
│   ├── market-open/
│   ├── mid-day/
│   ├── power-hour/
│   └── after-hours/
├── event-rules/
│   ├── fomc/
│   ├── earnings/
│   ├── cpi-release/
│   ├── fed-speaker/
│   └── macro-data-cluster/
├── performance/
│   ├── win-loss-tracking/
│   ├── strategy-metrics/
│   ├── drawdown-analysis/
│   └── slippage-and-fees/
├── safety/
│   ├── risk-parameters/
│   ├── regime-gates/
│   ├── kill-switches/
│   └── max-exposure-rules/
└── strategy-variants/
    ├── baseline/
    ├── vix-filter/
    └── sector-rotation-filter/
```

#### Node Definitions
| Node | What gets stored | Update trigger | Primary querier | Example content |
|---|---|---|---|---|
| `trading-patterns/` | Index of approved pattern playbooks | New pattern family added | Pre-trade planner | "Allowed pattern families: breakout, pullback..." |
| `trading-patterns/double-top` | Entry criteria, invalidation, stop logic | Pattern review or repeated outcomes | Signal validator | "Require second peak rejection + volume divergence" |
| `trading-patterns/head-shoulders` | Neckline rules, confirmation rules | After 10+ observed occurrences | Setup selector | "Short below neckline only if market breadth weak" |
| `trading-patterns/breakout` | Breakout quality filters, volume thresholds | Weekly tuning | Execution planner | "Ignore low-float names unless spread < 8 bps" |
| `trading-patterns/pullback-continuation` | Trend pullback entry templates | Strategy refinement | Setup selector | "Buy first higher-low at VWAP in trend regime" |
| `trading-patterns/failed-breakout` | Reversal criteria after failed break | Post-mortem cluster | Risk manager | "If reclaim fails within 15m, reverse bias" |
| `trading-patterns/gap-and-go` | Gap magnitude bands, opening range behavior | Monthly stats update | Market-open module | "Only trade if opening drive confirms > premarket high" |
| `market-regimes/` | Regime taxonomy and detection logic | Detection model update | Router | "Regime inference priority: volatility > trend > breadth" |
| `market-regimes/trending` | Conditions/filters for trend days | Regime reclassification | Pre-trade checks | "ADX > 22, pullbacks shallow, breadth aligned" |
| `market-regimes/ranging` | Mean-reversion conditions | Ranging lessons curated | Setup filter | "Avoid breakout continuation in low ATR compression" |
| `market-regimes/volatile` | Volatility guardrails | VIX/ATR shifts | Risk gate | "Reduce size 40%, widen stop only with lower leverage" |
| `market-regimes/low-volume` | Liquidity requirements and skip rules | Seasonal liquidity changes | Execution engine | "Skip symbols with avg 1m vol below threshold" |
| `market-regimes/risk-on-risk-off` | Correlation and breadth gates | Macro risk regime change | Portfolio allocator | "Risk-off: cap gross exposure at 0.6x baseline" |
| `time-setups/` | Session segmentation policy | Market-hours policy changes | Scheduler | "Session windows and allowed setup families" |
| `time-setups/pre-market` | Premarket scan criteria | Daily premarket review | Scanner | "Gap >= 2%, RVOL >= 1.8 for watchlist" |
| `time-setups/market-open` | Open-drive playbook | Open-session analysis | Execution planner | "First 5m no-chase rule unless liquidity score high" |
| `time-setups/mid-day` | Mid-day risk reduction rules | Ongoing performance drift | Position manager | "Halve target; avoid fresh breakouts in chop" |
| `time-setups/power-hour` | Late-session momentum rules | Monthly review | Trade manager | "Permit breakouts only with closing auction strength" |
| `time-setups/after-hours` | AH trading constraints | Broker/liquidity policy changes | Safety checker | "AH entries disabled unless event-driven exception" |
| `event-rules/` | Event calendar handling baseline | Calendar source changes | Event router | "All high-impact events require event mode" |
| `event-rules/fomc` | FOMC day no-trade windows and re-entry | FOMC post-mortem | Risk gate | "No new risk 15m pre/post statement" |
| `event-rules/earnings` | Earnings proximity restrictions | Symbol-level earnings updates | Symbol filter | "No swing entries within T-1 to T+1 for earnings" |
| `event-rules/cpi-release` | CPI timestamp volatility controls | Macro event review | Risk gate | "Reduce size to 25% during first release impulse" |
| `event-rules/fed-speaker` | Speech-window constraints | Event feed updates | Event filter | "Pause discretionary entries during unscheduled remarks" |
| `event-rules/macro-data-cluster` | Multi-event overlap handling | Calendar conflict detection | Scheduler | "If CPI+jobless same session, use defensive profile" |
| `performance/win-loss-tracking` | Aggregated W/L by setup-regime-time | Daily close | Analytics agent | "Breakout in trend @ open: 58% win rate (30d)" |
| `performance/strategy-metrics` | Sharpe-like stats, expectancy, PF | Daily/weekly recompute | Strategy evaluator | "Expectancy improved after VIX filter by +0.08R" |
| `performance/drawdown-analysis` | Drawdown episodes + causes | New drawdown threshold breach | Risk committee agent | "Largest DD from overtrading in ranging midday" |
| `performance/slippage-and-fees` | Cost model observations | Broker fill-quality update | Execution optimizer | "Avoid entries with spread > 12 bps in first minute" |
| `safety/risk-parameters` | Hard caps: max risk/trade/day/week | Risk policy update | Risk engine | "Max daily loss 2R; max concurrent positions 4" |
| `safety/regime-gates` | Regime-dependent allow/deny matrix | Regime policy revisions | Pre-trade checker | "Ranging + low volume => disable breakout strategy" |
| `safety/kill-switches` | Emergency stop conditions | Trigger threshold changes | Safety monitor | "3 losses + abnormal slippage => halt until review" |
| `safety/max-exposure-rules` | Exposure by symbol/sector/beta | Portfolio framework changes | Allocator | "Per-sector cap 30%, single-name cap 12%" |
| `strategy-variants/baseline` | Canonical active strategy spec | Baseline update | Variant comparator | "Baseline v1.4 ruleset and metrics snapshot" |
| `strategy-variants/vix-filter` | Experimental VIX-gated rules | New experiment cycle | Research agent | "Trade only when VIX < 25" |
| `strategy-variants/sector-rotation-filter` | Sector-strength constraints | Rotation model update | Research agent | "Only long in top-2 relative strength sectors" |

#### Curation Quality Bar
- Promote to ByteRover only if reusable across multiple sessions.
- Keep entries concise, testable, and tied to measurable outcomes.
- Link each strategy variant node to performance evidence before adoption.
