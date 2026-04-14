### soul.md Preservation Strategy

#### Goal
Persist Lana Sterling's personality, memory continuity, and trading knowledge across container rebuilds without violating security boundaries.

#### Persistence layers
1. **Identity layer (personality/core behavior)**
   - Canonical file: `/workspace/persistence/soul/soul.md`
   - Companion checksum: `/workspace/persistence/soul/soul.md.sha256`
   - Version log: `/workspace/persistence/soul/versions.jsonl`

2. **Operational memory layer**
   - SQLite snapshots: `/workspace/persistence/memory/sqlite/`
   - Mem0 export/import bundles: `/workspace/persistence/memory/mem0/`

3. **Structured knowledge layer**
   - ByteRover curated content under project tree:
     `alpaca-pattern-engine/{trading-patterns,market-regimes,time-setups,event-rules,performance,safety,strategy-variants}`

#### Backup and restore lifecycle
- **Pre-shutdown hook**
  1. Validate `soul.md` schema + checksum.
  2. Snapshot SQLite.
  3. Export Mem0 memory bundle.
  4. Run ByteRover curation flush.
  5. Append manifest: `/workspace/persistence/manifests/<timestamp>.json`

- **Post-start hook**
  1. Restore latest valid manifest.
  2. Verify checksum/signature for `soul.md`.
  3. Import SQLite + Mem0 snapshots.
  4. Run `brv query` smoke test on key nodes.
  5. Emit restore report.

#### Suggested soul.md sections
- `Identity`: Lana Sterling persona commitments and communication style
- `Mission`: autonomous paper-trading goals and boundaries
- `Non-Negotiable Safety`: hard limits copied from `safety_config.toml`
- `Operating Preferences`: mode-selection heuristics (Solo/Delegate/Swarm)
- `Reflection Journal`: concise lessons and adaptation notes

#### Integrity and anti-drift controls
- Require checksum match before loading persona.
- Keep append-only revision history with reason metadata.
- Reject unsafe edits that conflict with hard safety constraints.
- Auto-reconcile `soul.md` safety section with `safety_config.toml` at startup.

#### Cross-container storage policy
- Store persistence data on Docker volume mounted at `/workspace/persistence`.
- Never rely on container-local ephemeral paths for identity/memory data.
- Encrypt archives at rest if external storage sync is enabled.

#### Recovery modes
- **Full restore:** identity + memory + knowledge all available.
- **Partial restore:** identity + SQLite available, Mem0/ByteRover degraded.
- **Safe minimal mode:** load identity + safety only, disable autonomous execution until memory layers recover.
