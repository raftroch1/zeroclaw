### Three-Layer Memory Architecture for ZeroClaw + Lana Sterling

#### Objective
Use the right memory system for the right retrieval pattern. ByteRover is an additional structured reasoning layer on top of existing SQLite + Mem0; it does **not** replace either layer.

#### Layer Roles
- **SQLite (exact/transactional memory)**
  - Best for: trade records, session state, flags, parameters, deterministic lookups.
  - Characteristics: low-latency, precise filters, SQL joins/aggregations.
- **Mem0 (semantic episodic memory)**
  - Best for: lessons learned, narrative post-mortems, fuzzy recall across prior situations.
  - Characteristics: semantic search, natural-language retrieval.
- **ByteRover (structured project knowledge graph/tree)**
  - Best for: canonical rules, regime-specific playbooks, pattern checklists, strategy variants.
  - Characteristics: curated context tree + versioned knowledge operations.

#### Decision Tree (Routing)
```text
Start
 ├─ Need exact numeric/state answer from a known record?
 │    └─ Yes → SQLite
 │
 ├─ Need fuzzy recall of prior experiences/lessons in natural language?
 │    └─ Yes → Mem0
 │
 ├─ Need structured, reusable strategy knowledge/checklist by domain?
 │    └─ Yes → ByteRover
 │
 └─ Need multiple? → Query in this order:
      1) ByteRover (policy/checklist)
      2) SQLite (current facts)
      3) Mem0 (analogous lessons)
```

#### Concrete Query Routing Examples
- **"What should I check before trading?"** → **ByteRover**
  - Pull current regime gates, pattern preconditions, and time-of-day rules.
- **"What was my last trade result?"** → **SQLite**
  - Deterministic query by timestamp/account/symbol.
- **"What lessons did I learn from ranging markets?"** → **Mem0**
  - Semantic retrieval over past narrative insights.

#### Data Flow from Trade Lifecycle
1. Trade executes and closes.
2. Store raw immutable trade data in **SQLite**.
3. Generate lesson summary (win/loss reason, mistake, strength).
4. Write lesson text to **Mem0** for semantic recall.
5. Curate durable structured guidance to **ByteRover**:
   - update pattern node,
   - update regime gate notes,
   - update performance node if statistically meaningful.

#### Routing Algorithm (Agent-Side)
```pseudo
function route_memory(query, intent, payload):
  if intent in ["trade_lookup", "state_lookup", "metrics_sql"]:
      return SQLITE

  if intent in ["lesson_recall", "retrospective", "analogy"]:
      return MEM0

  if intent in ["pre_trade_checklist", "strategy_rule", "regime_policy", "setup_playbook"]:
      return BYTEROVER

  # fallback: hybrid
  result_brv = query_byterover(query)
  result_sql = query_sqlite_if_needed(query)
  result_mem0 = query_mem0_if_needed(query)
  return synthesize(result_brv, result_sql, result_mem0)
```

#### Retrieval/Write Policies
- **SQLite write policy:** every execution event and portfolio state mutation.
- **Mem0 write policy:** every post-trade lesson and notable qualitative observation.
- **ByteRover write policy:** curated, reusable, high-signal knowledge only.
  - Avoid dumping raw trades.
  - Prefer normalized entries under stable tree nodes.

#### ASCII Architecture Diagram
```text
                         +----------------------+
                         |    ZeroClaw Agent    |
                         |   (Lana Sterling)    |
                         +----------+-----------+
                                    |
                      Intent Router / Memory Broker
                                    |
        +---------------------------+---------------------------+
        |                           |                           |
+-------v--------+          +-------v--------+          +-------v--------+
|    SQLite      |          |      Mem0      |          |   ByteRover    |
| exact records  |          | semantic recall|          | structured tree|
| trades/state   |          | lessons/stories|          | rules/playbooks|
+-------+--------+          +-------+--------+          +-------+--------+
        |                           |                           |
        +------------+--------------+--------------+------------+
                     |  Synthesized Decision Context |
                     +-------------------------------+
```

#### Operational Guidance
- Keep all three layers active.
- If ByteRover is down, continue with SQLite + Mem0 and log degraded mode.
- Promote high-confidence recurring insights from Mem0 into ByteRover periodically.
