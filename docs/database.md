# CSV database format (schema 1)

```
mordheim-db/
  LOCK                    advisory OS lock; crash releases it
  CURRENT                 committed generation UUID, followed by newline
  generations/<UUID>/     immutable CSV database image for one transaction
    manifest.csv          filename and SHA-256 digest for each table
    campaigns.csv
    warbands.csv
    profiles.csv
    types.csv
    items.csv
    permissions.csv
    equipment_lists.csv
    equipment_list_items.csv
    equipment_list_assignments.csv
    abilities.csv
    caps.csv
    thresholds.csv
    units.csv
    members.csv
    advances.csv
    effects.csv
    holdings.csv
    allocations.csv
    ledger.csv
    notes.csv
    battles.csv
    participants.csv
    steps.csv
    searches.csv
    resolutions.csv
    dice.csv
    snapshots.csv
    events.csv
```

Rows use stable primary keys (`id`); names are display fields, not foreign keys.
CSV quoting preserves commas, quotes, Unicode and multiline notes. Boolean values
are `true`/`false`, numbers use decimal integers, and blank optional references
mean absent. Empty tables may be zero-byte files. Missing original tables are errors.
The three equipment-list tables are additive and default to empty for legacy
CSV/JSON inputs. New generations always write and checksum all three.
`src/model.rs` defines exact field names and types. JSON import uses those same
records as arrays named after each table and rejects unknown fields.

- `campaigns` → `warbands` → `units` → `members` is the ownership hierarchy.
  A unit is either an individual's progression record or a shared Henchman group.
  Each model is a member, including individuals in a group. A promoted model
  keeps its member ID and moves to a new Hero unit.
- `profiles` → `types`, `items`, `abilities`, `thresholds` is the rules hierarchy.
  Type stats are strings supporting numeric or random values. `caps` references
  a type/characteristic. `permissions` links a type (or promoted Hero unit) to
  abilities: kind `ability` or `starting-ability`. Kind `starting-item` records
  an included equipment item and `quantity` per model on recruitment. Quantities
  default to one for older CSV headers/JSON; starting type/item pairs are unique.
  Legacy `item` permissions are converted when data is loaded/imported.
- `equipment_lists` defines named, profile-scoped lists. `equipment_list_items`
  joins lists to items with unique `(list_id, item_id)` pairs; one item may be on
  several lists. `equipment_list_assignments` has one `(entity_id, list_id)` per
  warrior type, unit or member. A member's effective list is its explicit
  assignment, otherwise its unit's assignment, otherwise its type's default.
  Multiple types may share one list. Recruits inherit immediately; promotion
  preserves group overrides and rules migration maps overridden list items.
  Assignments/items must belong to the same profile. Shared-list changes retain
  existing allocations; future group equip actions check every active member.
- Legacy type permissions become deterministic shared lists for equal item sets
  within a profile. Unit-specific additions become separate lists containing the
  type list plus those additions. Skill permissions remain separate. Upgrade is
  idempotent and happens in memory; source generation files/manifests remain
  unchanged. The next ordinary save persists the new tables. Legacy manifests
  without these tables remain readable; removing a checksummed new table is an
  integrity error. Old backups/imports receive the same conversion. The hidden
  legacy `add-equipment --allowed` argument maps targets to their assigned lists;
  new commands and the TUI select lists directly.
- `advances` references a unit, optional ability/dice, and a threshold. Threshold 0
  represents a pre-existing/initial increase or immediate promotion advance.
  A placeholder with reason `pending-promotion` is replaced by the resolved
  advance in a subsequent audited generation. `effects` references individual
  members and optionally a battle; contextual effects have a condition string.
- `holdings` has one row per warband/item. `allocations` has one row per
  member/item. Owned quantities cannot be less than the sum of allocations;
  stash is the difference. Allocation never creates inventory.
- `ledger` retains actual gold changes, quantities, prices, dates, phase,
  optional member/item/battle references and reasons. Catalog price changes do
  not rewrite historical transactions. Treasury is the sum of gold changes.
- `battles`, `participants`, ordered `steps`, `searches`, and unique keyed
  `resolutions` retain battle state. Consumption changes inventory immediately.
  Pending stages can be resolved or explicitly skipped in a chosen order;
  each action retains eligibility, resource, pending-outcome and duplicate checks.
  `searches.item_id`, `outcome` (found/failed), and `purchased` link each rare-item
  search to one matching purchase. Heroes taken out of action remain ineligible
  even after injury recovery. Failed searches still spend the Hero's one attempt.
- `campaigns.skip_dramatis_personae` persists the campaign-wide house rule.
  It automatically skips and hides Dramatis Personae for all campaign warbands;
  completed battles are never rewritten. The default for older data is false.
  These additive campaign/search columns use deserialization defaults, so older
  CSV headers and JSON opening states remain readable without editing generations.
- Kill/OOA credits are individual `resolutions` with keys
  `kill:PARTICIPANT-ID:CREDIT-ID`; `description` holds the opponent name/description
  (blank means unspecified). `participants.kills` remains the total and may include
  additional unspecified credits from older data. Named entries cannot exceed
  the total; counter edits cannot silently drop them. Removing a credit updates
  the total and current resolution rows atomically; previous generations retain it.
  No new CSV tables or columns are required. Other outcomes cannot use this prefix.
- `participants.attribution` is retained for compatibility and displayed as a
  model note. New observations use `notes`; the legacy CLI flag also appends a
  model note. Counter shortcuts never clear legacy text or duplicate it into notes.
- `notes` attach to an entity; `selection` is the reserved entity ID for the
  preferred warband. Note replacement appends a superseding note. Recent previews
  show three current entries in recorded order, excluding superseded versions;
  full notes and historical generations retain all versions.
- `snapshots` name a generation and warband context. `events` record append
  sequence, recorded-at UTC time, effective date, action, related IDs, reason,
  provenance and parent/resulting generations. Effective dates do not control
  replay order. Imported unknown historical dates may be blank in domain rows;
  every new event has a real timestamp and valid effective date.

A commit builds fresh tables in `staging-UUID`, flushes/syncs them, calculates a
manifest, syncs the directory, renames it into `generations`, and atomically
replaces `CURRENT` after syncing it. On restart only the generation named by
`CURRENT` is authoritative. Unreferenced staging/generation folders from an
interruption are ignored. No automatic cleanup discards historical records.
Checksums detect unintended edits, not malicious tampering by someone who can
rewrite the database and manifest. Bundle backups provide portable recovery.

History uses complete relational state images for deterministic replay, rather
than requiring old versions of command logic to reconstruct state. This costs
disk space proportional to database size times number of commits; suitable for
local warband bookkeeping. A future compactor must preserve referenced history.
Schema changes must ship explicit migrations; this version rejects unsupported
profile schema versions and invalid/unknown table fields.

Rule folders contain profiles, types, items, permissions, abilities, caps and
thresholds, plus equipment_lists, equipment_list_items and
equipment_list_assignments. The latter three remain optional for legacy folders. Author new IDs/content versions for
changes. Validate with `mordheim rules FOLDER`; errors identify the table or
invalid reference. Source and classification are attached to profiles, with
ability source/description fields for specific mechanics. Scenario, injury,
exploration and advance-result tables beyond progression thresholds are currently
player-resolved, recorded as consequences via advances and keyed resolutions.
Catalog definitions can also be added through `add-equipment` / `add-ability`
and the TUI. They append current-profile entries and explicit type/unit permissions
without rewriting earlier generations or external rule files. `grant-ability`
records a learned skill as a threshold-zero advance; normal earned advances
continue to consume their own pending thresholds.
Dice rows and links remain readable for older records, but new workflows do not
require them. Veteran availability has a direct numeric entry path. Data files contain no
executable rules or scripts.

AI proposals contain ordinary CLI arguments rather than database patches. Review
replays the complete sequence against a disposable bundle, including historical
generations for undo/restore. Approval is bound to the reviewed CURRENT and consumed
once. Publication adds validated immutable generations and switches CURRENT once;
each ordinary action retains its AI-assisted event. Existing generations must
match exactly. Export artifacts are kept in the disposable directory until approval;
file conflicts or stale approvals do not publish the batch. Provider credentials
and workbench conversation state are not written to campaign tables.


The Rules editor and `rulebook` commands can author these records directly,
without an existing warband or external rules folder. `rulebook schema` prints
strict JSON templates; `rulebook save` accepts a tagged `{kind,data}` definition.
An empty ID creates a record and an existing ID edits it. Profile copies remap
all catalog, permission, cap, progression and list references. Definition deletion
checks dependencies; edits never overwrite historical generation files.

Profiles and warrior types have an optional `description` column for manually
adjudicated rules. Warrior `leader` and `free_dagger` columns are optional booleans:
blank legacy values retain the Captain/free-dagger conventions; authored values
are explicit. Starting equipment grants are separate from list eligibility and
use zero-cost find ledger entries. Included items cover reinforcement loadout
needs before consuming stash; any surplus remains unassigned. Changing defaults
retains existing allocations, transactions, earned XP/advances and innate grants.
