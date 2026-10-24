# Disclaimer
This application is 100% vibe-coded. Use at your own risk.

# Mordtui

Mordtui is an offline Mordheim campaign manager for Linux, with a terminal user
interface and a command-line interface. Track warbands, shared Henchman groups,
equipment, gold, experience, advances, injuries and battle history in one local
CSV database.

Players roll dice and resolve tabletop rules in person; Mordtui records the
consequences. Core campaign management needs no account, network connection or AI
service. The executable is named **`mordheim`**.

## Get started

You need Rust and Cargo, plus an interactive terminal of at least **80 columns ×
24 rows**. The project uses the Rust 2024 edition. Run the following from the
repository root:

```sh
cargo build --locked --release
./target/release/mordheim
```

Running without a command opens the TUI. An empty database offers campaign
creation, import and Rules-editor actions. The default database is `mordheim-db/` relative to
your working directory; use `--db PATH` to choose another location.

To install the executable on your Cargo binary path:

```sh
cargo install --path . --locked
mordheim --help
```

The remaining examples use the installed `mordheim` command. You can substitute
`./target/release/mordheim`, or use `cargo run --locked -- COMMAND` during development.
Example file paths below are relative to the repository root.

### Try the example warband

Import into a fresh database. The first command previews the import; `--apply`
saves it.

```sh
mordheim --db demo-campaign import examples/burgomeisters.json
mordheim --db demo-campaign import examples/burgomeisters.json --apply
mordheim --db demo-campaign
```

The example is a documented transcription of the original tracking spreadsheet:
**12 models, rating 122, 2 gold crowns**, and all owned equipment issued. It
retains the campaign's custom starting funds, advances and notes.

### Start a new campaign

```sh
mordheim --db my-campaign init --rules examples/rules --profile reikland \
  --campaign 'City of the Damned' --name 'The Watch'
mordheim --db my-campaign
```

Use **H Hire** on the Warband tab to recruit your first warriors. The supplied
profiles cover minimal Mercenary, Reikland, Marienburg and spreadsheet house-rule
examples. Their equipment and ability catalogs are starting points rather than a
complete rules library.

## Using the TUI

The header always identifies the current phase: **PRE-BATTLE**, **BATTLE**, or
**POST-BATTLE**. A prepared battle also shows **READY**. Every accepted change is
saved immediately, with a journal entry.

| Tab | Purpose | Common controls |
| --- | --- | --- |
| Overview | Treasury, rating, availability, upkeep and session tasks | **F2** switch warband, **F5** battle |
| Warband | Spreadsheet-style profiles, group sizes, XP and pending advances | **H** hire, **V** veteran XP, **D** dismiss, **E** equip, **X** XP, **U** advance, **G** equipment lists |
| Stash | Owned, issued and stored equipment, prices and owners | **P** purchase, **S** sell, **G** item lists, **C** new equipment |
| Battle | Individual profiles, wounds, condition, kill credits and objectives | **−/+** wounds, **K/J** add/remove kill, **O** out of action, **W** counters, **V** full profile |
| Post-battle | Stages alongside their relevant models, groups and outcomes | **C** finish stage, **K** skip stage; other keys follow the selected stage |
| Journal | History, snapshots, injuries, ledger and notes | **V** change journal view, **S** snapshot, **Enter** details |
| Rules | Author profiles, warrior types, lists, equipment, skills, XP progression and starting gear | **P** profile, **N** new, **E** edit, **C** copy, **D** delete, **W** create warband |
| AI assistant | External-agent tasks, proposals and player approval | **N** new task, **P** next proposal, **R** review, **Y** approve, **D** reject |

Use **Left/Right**, numbered tabs, or **Tab/Shift-Tab** to change views. On
Post-battle, Tab/Shift-Tab switches focus between stages and targets; on Rules it
switches between sections and definitions;
Left/Right still changes the main tab. **Up/Down** selects rows, **/** filters,
**Enter** opens details or the selected action, and **A** opens the searchable
Actions menu. Tabs, rows, fields and buttons also accept mouse clicks.

Global shortcuts are **F2** warband selection, **F3** campaign rules, **F4** the
injury list, **F5** prepare/start/end battle, **F6** model/unit notes, and **F7** the Rules editor. Press
**?** for help and **Q** to quit. The optional **:** command prompt is available
for advanced use; routine session tasks have forms and buttons.

Warband stat labels repeat above each group's values. Pending advances are
marked **READY**. Selecting or hovering over a model shows its three most recent
notes; F6 opens the full note history. Dismissal lets you return carried equipment
to the stash or record its loss, while retaining the member's history.

When reinforcing a Henchman group, select **Reinforce group** in **H Hire**.
The review shows the hire price, veteran premium, included gear, stash copies,
automatic purchases and total treasury change. Missing equipment is bought at
catalog prices if common and permitted by the group's equipment list; rare items
must already be in the stash. The entire hire and its equipment are saved together.
The default hire price per recruit is **base cost + 2 gc × the group's XP**;
equipment purchases are additional. The optional actual unit price replaces that
hire charge and leaves equipment costs separate.

Experienced reinforcements use the current post-battle allowance, or the most
recent completed battle's remaining allowance between battles. **V Veteran XP**
on Warband lets you record or correct the physical roll's **total**, even after
that battle is complete. **V** is also available on Post-battle after completion
and in recruitment. Already spent XP stays spent; correcting the total does not
reopen stages or change saved snapshots. A prepared or active battle still locks
recruitment and allowance changes.

### Forms and selections

Forms start with the selected model, group or item. The first typed input
replaces a prefilled value. Move the cursor first to edit that default in place.
Tab/Shift-Tab or Up/Down moves between fields; Page Up/Down jumps through long
forms. Enter opens a searchable picker.

For multiple-choice pickers, **Space/click** toggles checkboxes, **Enter/Done**
accepts, and **Esc** discards picker changes. Filtering retains checked choices;
**Ctrl-A** selects all shown and **Ctrl-U** clears those shown.

**F10** or **Ctrl-S** opens the review; **Enter** then applies the action. Errors
retain your entries. Forms generate a readable journal reason from the action
and selected values, which you can replace. Relevant profile exceptions have
specific controls; inventory and ownership checks still apply.

### Program a warband from scratch

Open **7 Rules** or press **F7**, including from an empty database. The editor
has sections for profile defaults, warrior types, equipment lists, equipment,
skills/spells, XP progression and starting gear. Use **Up/Down** to choose a
section; **Tab** switches focus to its definitions. **N** adds, **E** edits,
**V** expands details, and **D** removes an unused definition. **P** switches the
profile being edited independently of the warband you are playing.

1. In **Profile defaults**, create a profile with its faction, version, source,
   classification, starting gold and bonus, roster/Hero limits, debt setting,
   initial-advance cost and campaign-specific rule descriptions.
2. In **Warrior types**, define Heroes, Henchmen, Hired Swords or Dramatis
   Personae. Set all nine base characteristics, hire cost, upkeep, starting XP,
   whether XP is earned, recruitment/group limits, rating contribution and
   roster/income/battle participation. Choose its equipment list, learnable and
   innate skills, characteristic caps, leader setting and special rules.
3. Define items and equipment lists, then select the types/items sharing each
   list. Skills, spells and prayers have a category, source, rules text, and
   learnable/innate type assignments. Definitions can be added in any order;
   a new type without a chosen list receives an empty one to fill later.
4. Set **XP progression** thresholds for each role. Under **Starting gear**, add
   the items and quantities included with each recruited model. These create
   zero-cost acquisitions; reinforcements retain the group's existing loadout
   and surplus starting items go to stash. The warrior form also controls the
   legacy free-dagger convention.
5. Press **W Create warband** to use the stored profile in an existing or new
   campaign. Starting funds and bonuses are recorded once; hire models afterward.

**C Copy profile** creates an independent copy with all definitions and references
remapped. Editing a shared profile changes its current users; new funding,
starting XP, starting equipment and innate abilities apply to future warbands or
hires. Transactions, existing equipment and earned advances are retained, as are
previous generations and frozen battle references. Definitions referenced by
rosters, learned skills, inventory or transactions cannot be deleted.

Special recruitment conditions, contracts, injuries and other tabletop mechanics
can be recorded in rule descriptions. Players adjudicate these rules and enter
their consequences; descriptions are not executable scripts. No rules files or
existing campaign are needed to author a profile.

### Equipment lists and skills

Equipment belongs to named **equipment lists**. Several warrior types can share
a list, and one item can belong to multiple lists. Models use their individual
assignment, otherwise their group's assignment, otherwise their type's default.
New recruits inherit the appropriate group/type list.

- **A → Create shared equipment list** defines a list and selects multiple
  warrior types and existing items at once.
- **G** on Warband assigns lists to types, groups or models. Choose **Inherit
  default** to clear a group/model override.
- **C** on Warband or Stash defines equipment. Check every list that should
  receive the item. **G** on Stash edits an existing item's list memberships.
- Purchase/equip pickers offer **New equipment**, then return to the original
  form after you save its definition.
- **S** on Warband defines a skill, spell or prayer. **L** records a learned
  skill without spending a pending advance; **U** resolves an XP-earned advance.

Catalog creation and purchasing/equipping are separate actions. List changes
apply to everyone using the shared list, while existing carried gear and
historical references are retained. Group equip actions check every active
member's list and require enough copies for the whole group.

### A battle session

1. Prepare the roster and equipment, then use **F5** to prepare and start a
   battle. The pre-battle roster is saved as a frozen reference.
2. On Battle, select individual models to record wounds, condition and
   out-of-action results. **Rem** is wounds remaining, separate from the **W**
   characteristic. **K** records a kill/OOA credit with an optional opponent name;
   **J** removes a chosen credit. **]/[** adds/removes objective credits.
3. Use **F5** to end the battle and record its result. Out of action does not
   imply death; injuries are resolved explicitly afterward. Kill credits do not
   award XP automatically.
4. Resolve injuries, XP/advances, exploration, shard sales, searches, recruitment,
   upkeep and equipment in Post-battle. Browse stages freely; each action checks
   its own prerequisites. **C** finishes a stage, while **K** records a skip.
   Finishing the last unresolved stage also completes the battle and saves its
   checkpoint.

Rare-item trading allows one search for each participating, active Hero who was
not taken out of action. Record the specific item and the physical roll's
found/failed consequence; a success permits one matching purchase. Wyrdstone
sales use the campaign's agreed total price, including zero shards sold.
**F3 → Skip Dramatis Personae** hides and automatically skips that stage across
the campaign's current and future battles.

## Command-line use

CLI and TUI actions share validation and persistence. Use `--help` or
`COMMAND --help` for arguments. Commands accept stable IDs and unambiguous names;
`--warband` and `--campaign` select a context when needed. `select NAME` remembers
a warband. `--json` produces machine-readable output.

```sh
mordheim list campaigns
mordheim list warbands
mordheim list roster --filter Hans
mordheim --json list equipment
mordheim list equipment-lists
mordheim show Hans
mordheim note Hans 'Paint the shield'
mordheim --date 2026-10-12 --reason 'Scenario award' xp Hans 1
```

For a fresh campaign, the same operations are available directly:

```sh
mordheim --db my-campaign recruit Captain Hans
mordheim --db my-campaign recruit Marksmen Arrows --count 2
mordheim --db my-campaign buy Sword 2
mordheim --db my-campaign equip Arrows Sword
```

Run a battle with `battle prepare`, `battle start`, `battle record`, `battle kill`
and `battle end`. Use `battle awards` to inspect suggested XP, then record awards
and advance consequences explicitly. `step NAME` completes a post-battle stage;
`step NAME --skip` records a skip. Use `wyrdstone 0 0` to record no sale.

## Saves, history and backups

A database folder holds normalized CSV tables and immutable generations. Writes
use an atomic `CURRENT` pointer, checksums and a single-writer lock. Close the TUI
before opening the same database from another process. Failed commands leave
campaign state unchanged.

```sh
mordheim snapshot 'Before shopping'
mordheim diff 'Before shopping'
mordheim --reason 'Wrong amount' undo
mordheim --reason 'Return to checkpoint' restore 'Before shopping'
mordheim list history
mordheim export roster.md
mordheim export ledger.csv --format ledger
mordheim export inventory.csv --format inventory
mordheim export backup.json --format bundle
mordheim --db recovered-campaign recover backup.json
```

**Undo and restore affect the whole database**, including other warbands in that
folder. A bundle contains retained history and pinned rules and is the portable
backup format; recover it into an empty database. Exports require a new output
path.

For deliberate data edits, export a copy with `export editable --format csv` or
`export state.json --format json`, edit the copy, then preview
`import editable --reconcile`. Apply it with a reason and `--apply`. Saved
generations should not be edited in place. Older equipment permissions are
converted to shared lists when loaded without rewriting historical files.
See [the database reference](docs/database.md) for schema and reconciliation details.

## External AI agents (MCP)

Tab **8 AI assistant** is a local workbench for an external agent. Mordtui does
not call a model provider or require API credentials. The agent application
chooses its own model and manages its own conversation.

Add Mordtui to your agent application's MCP server configuration, using the same
**absolute database path** as your TUI:

```json
{
  "mcpServers": {
    "mordtui": {
      "command": "mordheim",
      "args": ["--db", "/absolute/path/my-campaign", "mcp"]
    }
  }
}
```

The executable must be on the agent application's PATH; otherwise use its absolute
path as `command`. The server uses MCP stdio, can run alongside the TUI, and works
in the default installation. Existing provider settings (`MORDTUI_AI_*`) are no
longer used. The old `ai` Cargo feature remains an empty compatibility alias.

In tab 8, **N** queues a local task. You can also queue one while the TUI is open:

```sh
mordheim --db /absolute/path/my-campaign ai 'Suggest equipment for my heroes'
```

Ask your agent to read `mordtui_requests`, `mordtui_context` and
`mordtui_commands`, then submit a complete plan with `mordtui_propose`.
`mordtui_rules` exposes unused rule profiles for authoring a new warband.
`mordtui_proposal_status` reports review feedback and the player's decision.
Agents can propose every ordinary player command, including rules, battle actions,
purchases, notes and history operations.

**P** cycles received proposals; the target warband is shown on each one.
**R** validates all actions in a disposable database and lists their actual
changes. **Y** opens an approval confirmation; press **Y** there to apply the
reviewed plan. **D** rejects it. No MCP tool can execute or approve actions.
Proposals survive restarts, apply once, and expire when campaign data changes.
Receiving and reviewing plans makes no campaign changes. The external agent is
responsible for any network use; Mordtui itself communicates only locally.

See [the external-agent protocol](docs/external-agents.md) for schemas, tool
arguments and the approval workflow.

## Development and reference

```sh
cargo fmt --check
cargo test --locked --no-default-features
cargo test --locked --all-features
cargo clippy --locked --all-targets --no-default-features -- -D warnings
cargo clippy --locked --all-targets --all-features -- -D warnings
```

Tests cover the TUI with a terminal test backend, CLI workflows, equipment-list
inheritance, inventory conservation, progression, battles, persistence, migration,
backups and AI proposal approval. Automated tests do not need a model provider or agent application.

| Path | Contents |
| --- | --- |
| [`src/tui/`](src/tui/) | Forms, views and session interactions |
| [`src/rules/editor.rs`](src/rules/editor.rs) | Rule authoring, profile copies and dependency checks |
| [`src/engine.rs`](src/engine.rs) | Shared command execution and validation |
| [`src/model.rs`](src/model.rs) | Data records and invariants |
| [`src/store.rs`](src/store.rs) | CSV generations, locking, checksums and backups |
| [`src/ai/`](src/ai/) | MCP server, proposal inbox and player approval |
| [`examples/`](examples/) | Opening warband and editable rule profiles |
| [`tests/`](tests/) | Integration tests |
| [`specification/specification.txt`](specification/specification.txt) | Requirements, acceptance criteria and rules sources |
| [`docs/database.md`](docs/database.md) | Database format and editing workflow |

The current scope is local, player-assisted campaign bookkeeping. Complete rules
libraries, automated combat/table adjudication, direct XLSX import and shared
campaign servers are outside the implementation. Custom data and recorded
consequences support campaign-specific play.
