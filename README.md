# Mordtui

An offline Mordheim campaign manager for Linux. The `mordheim` executable provides
both a CLI and a Ratatui interface. Rules and campaign data live in CSV database
folders. Every successful change is saved with its date, reason and history.

```sh
cargo build --locked
./target/debug/mordheim --help
```

Start with your existing warband:

```sh
# Inspect the import before applying it
cargo run -- import examples/burgomeisters.json
cargo run -- import examples/burgomeisters.json --apply
cargo run -- list heros
cargo run -- list equipment
cargo run -- show Hans
cargo run -- tui
```

The included opening state reproduces the spreadsheet: **12 models, rating 122,
2 gold, no wyrdstone, and all owned equipment assigned**. It uses the confirmed
Reikland identity and retains the spreadsheet's custom initial funding and
advances. Unknown original dates and rolls remain unknown. Anonymous Heroes have
placeholder names. The old search reminder is an ordinary note.

To start from scratch in another folder:

```sh
cargo run -- --db my-campaign init --rules examples/rules --profile reikland \
  --campaign 'City of the Damned' --name 'The Watch'
cargo run -- --db my-campaign recruit Captain Hans
cargo run -- --db my-campaign recruit Marksmen Arrows --count 2
cargo run -- --db my-campaign buy Sword 2
cargo run -- --db my-campaign equip Arrows Sword
cargo run -- --db my-campaign list summary
```

`examples/rules` contains minimal Mercenary, Reikland, Marienburg and workbook
house profiles. These are editable starting points with limited equipment and
abilities, not a complete rulebook. They follow the reviewed errata's post-battle
order. All unimplemented table effects are resolved explicitly by the player.

## CLI and TUI

Use `COMMAND --help` for arguments. Select the database with `--db PATH` (default
`mordheim-db`). Multiple campaigns and warbands share a folder; use `--campaign`
and `--warband` to disambiguate, or `select NAME` to remember a warband. Full IDs
always work; names must be unambiguous. IDs of eight or more characters also work
as prefixes in typed member/type/unit lookups. `list heros` aliases `list heroes`.

```sh
mordheim list campaigns
mordheim list warbands
mordheim list roster --filter Hans
mordheim list types
mordheim list abilities
mordheim list equipment-lists
mordheim --json list equipment
mordheim note warband 'One search left in the old campaign; reminder only'
mordheim note Hans 'Paint the shield'
mordheim --date 2026-10-12 --reason 'Opponent witnessed roll' xp Hans 4
# Roll physically; record only the consequence:
mordheim advance Hans --stat WS
mordheim buy 'Lucky Charm' 1 --price 3
```

Actual prices and tabletop outcomes are entered directly. Optional notes explain
unusual choices; advances and shard sales do not require dice logs or reasons.
Explicit replacements of profile restrictions remain available where relevant;
they cannot create negative stock, duplicate ownership, or unsupported debt.
The legacy CLI `--override --reason` option remains for preset exceptions.

Every tab displays **PRE-BATTLE**, **BATTLE**, or **POST-BATTLE** in its header.
A prepared battle also shows **READY**. The TUI opens on a spreadsheet-style **Warband** table with all nine
characteristics, XP, pending advances, group size and equipment. Each group's
stat labels repeat directly above its values, even after scrolling. Owed plus-ups
are marked **READY** with a count and a summary banner. Select a row to
see its members, skills, effects, equipment and notes; **Enter** opens the full
dossier. At wider widths the table includes equipment per model. The minimum
interactive layout is **80 columns × 24 rows**.

| View | What it shows | Common actions |
| --- | --- | --- |
| Overview | Gold, shards, availability, rating, upkeep and session tasks | Switch warband, prepare battle, checkpoint |
| Warband | Heroes and Henchman groups with profiles and a selected dossier | **H** hire, **D** dismiss, **G** equipment lists, **E** equip, **X** XP, **U** advance, **S** new skill, **L** record skill, **N** note |
| Stash | Owned, issued and stored copies, prices and named owners | **P** purchase, **S** sell, equip/find/lose/return |
| Battle | Condensed M/WS/BS/S/T/W/I/A/Ld table, remaining wounds, condition, kills and objectives | **V** full profile, **W** counters, **−/+** wounds, **K/J** add/remove kill, **]/[** objectives, **F5** start/end |
| Post-battle | Stages beside selectable members/groups, XP, loadouts, searches and veteran budget | Only the selected stage's displayed keys work; **C** finishes it, **K** skips it |
| Journal | Opens on history; **V** cycles history, snapshots, injuries, ledger and notes | **N** note/update, **S** snapshot, **F4** injuries, inspect/compare/restore/export |
| AI assistant | Proposed actions and validated changes; nothing applied automatically | **N** ask, **R** review, **Y** approval confirmation, **D** reject |

Use **Left/Right** or **1–7** to switch views from any tab
(**Tab / Shift-Tab** also works outside Post-battle),
arrows to select rows, and
**/** to filter a table. Click tabs, rows, action buttons, form fields and choice
lists with the mouse. **A** opens a searchable menu of all session actions,
including recovery/leadership, promotion, search allowances, upkeep, treasury
adjustments, physical-roll consequences, snapshots, undo and exports. **F2**
selects a warband; this is
also available through the Actions menu. A new database offers graphical creation
and import buttons. No command syntax or copied IDs are needed for these tasks.
**F3** opens campaign rules. Enable **Skip Dramatis Personae** once to hide and
automatically skip that stage across the campaign's current and future battles.
**F4** opens the injury list from any tab: unresolved casualties, named outcomes,
lasting penalties, source battles and recovery. Warrior dossiers also list injuries.
**F5** prepares/starts/ends a battle from any tab (or opens unfinished after-action).
Select a model with the keyboard or hover over its row to preview its three most
recent notes. Group previews include individual and group notes. **F6** opens the
full history; Battle rows mark notes with **[N]**. **Rem** is current wounds left,
separate from W. **V** expands the current profile and frozen pre-battle reference.
Use **N Note** for observations about wounds or casualties; older attribution text
is retained as a readable model note.

**D Dismiss** on Warband fires the selected warrior. Choose whether carried gear
returns to the stash or is lost, then review and apply. No gold is refunded;
notes, identity and past battles stay in history. Choose a successor if the
leader leaves.

Equipment pickers offer **+ New equipment…**. Enter its name, cost, category,
rarity, description and eligible users, then save to resume your purchase/equip
form. **C** also defines equipment from Warband/Stash. Skill pickers similarly
offer **+ New skill / spell / prayer…**; **S** on Warband opens its definition form.
Equipment creation has an **Equipment lists** checkbox picker: check every list
that should receive the new item. Lists can be shared by several warrior types,
and one item can appear in several lists. In **A Actions**, use **Create shared
equipment list** to choose multiple types and existing items at once. **G Lists**
on Warband assigns a list to types, groups or individual models; on Stash it edits
the selected item's lists. Models inherit their group/type list unless individually
assigned; **Inherit default** clears a model/group override. New recruits inherit
their group's or type's list. Details show the applicable lists, and group equip
checks every member. Shared changes affect all warbands using those lists;
existing carried gear and historical references are retained.

Catalog creation does not buy, equip or teach the entry. **L** on Warband records
a learned skill without consuming XP; use **U Advance** for an earned plus-up.
Definitions are saved to the profile in audited generations; no CSV edits or
profile migration are required, and frozen/historical profiles remain intact.

Forms default to your selected record. **Tab / Shift-Tab** moves between fields;
**Enter** opens a name-based choice picker, where typing filters the choices.
The first typed input replaces a prefilled value. Later input edits your entry,
including after leaving and returning to the field. Use Left/Right or Home/End
before typing to edit a default in place; Backspace/Delete clears an untouched default.
Left/Right cycles single choices. In multiple-choice pickers, **Space/click**
toggles a checkbox, **Enter/Done** accepts, and **Esc** discards picker changes.
Typing filters without losing checks; **Ctrl-A** selects all shown and **Ctrl-U**
clears shown choices. Text fields support
arrows, Home/End, Delete/Backspace and Ctrl-U. **F10 or Ctrl-S** opens a review;
**Enter** applies once, and **Esc** returns to editing or cancels. Reviews show
costs, group copy counts and important effects. Lists, long forms, reviews and
dossiers scroll; failed submissions preserve entries and display an error.
Every mutation form supplies a readable reason from the action, named targets and values.
It updates as you enter data, and you can type a replacement. Accepted mutations
appear in Journal history; financial entries carry the same explanation in the ledger.

Battle wound/objective/OOA shortcuts save immediately and preserve unrelated counters.
**K** opens a kill/OOA credit: optionally name the opponent, then review and apply.
**J** lets you choose a credit to remove, including older unspecified kills;
**[** corrects an objective. Kill entries do not automatically award XP or record
opponent deaths. Counters stay nonnegative.
OOA is separate from death, which is resolved explicitly after the battle. All
other forms use the same engine validation and history as the CLI, with visible
date and optional notes. Specific preset-replacement controls appear only when
an entry conflicts with a profile limit; there is no universal override checkbox.
**?** shows help, and **Q** exits. The
optional **:** prompt remains for advanced commands. The database stays locked
while the TUI is open.

Post-battle starts with the stage list focused: **Up/Down** freely browses all
stages, including completed and future stages. **Tab / Shift-Tab** selects the
target or stage pane; Up/Down selects rows within it. An empty target list returns
arrow navigation to the stages. Click a stage and a target, or use **Enter** on a target
for its primary action. Injury and XP/plus-up forms target these rows directly.
Filtering and switching views retain stage context. Every pending stage has its
own actions, **C** to finish and **K** to skip, including when browsing ahead.
The displayed sequence is guidance; each action validates its own prerequisites.
Completed/skipped stages are read-only. Hotkeys apply only to the selected stage.

Rare-item trading gives each active participating Hero who was **not taken out
of action** one search. **S** selects the item and records **found/failed** from
the physical roll; either result spends that Hero's search. **P** buys one matching
item after a success. The list shows eligibility, remaining searches, results and
purchases. Scenario-specific allowances and the old spreadsheet reminder do not
control these searches.

**C** validates and finishes the current stage immediately, advancing to the next.
Finishing XP also confirms your allocation, so no extra confirmation result is
needed. **K** explicitly skips a stage through its review form. Completing rating
finishes the battle and saves the checkpoint. For advances and all other table
results, roll in person and record the consequence; no dice-link field is needed.

## Battle workflow

```sh
mordheim battle prepare 'The bridge' --scenario Skirmish --opponent Skaven --rating 160
mordheim battle start
mordheim battle record MEMBER-ID --condition out-of-action --kills 1
mordheim note 'The bridge' 'Hans held the doorway'
mordheim list battle
mordheim battle end win
mordheim battle injury MEMBER-ID recovery
mordheim step injuries
mordheim battle awards
# Allocate scenario/underdog/survival awards explicitly with xp; resolve advances.
mordheim step experience
mordheim resolve exploration 'Ruins searched; two shards' --shards 2
mordheim step exploration
mordheim wyrdstone 2 75
mordheim step wyrdstone
mordheim veterans 6
mordheim step veterans
mordheim rare-search Hans 'Lucky Charm' --found true
mordheim buy 'Lucky Charm' 1 --price 3
mordheim step rare
mordheim --reason 'No Dramatis Personae sought' step dramatis --skip
# Recruit/common purchases and hired-sword upkeep, if applicable.
mordheim step recruitment
# Allocate equipment here; group loadouts require enough copies for all members.
mordheim step equipment
mordheim step rating
mordheim battle complete
```

The campaign setting is also available with
`mordheim campaign-rules --skip-dramatis-personae true` (use `false` to disable).
It applies to every warband in the selected warband's campaign and is preserved
across restarts. Completed battles retain their original history.

Every stage can be resumed after restart. Explicitly skip irrelevant stages with
`step NAME --skip`; a note is optional. Pending advances and out-of-action injuries block
normal stage completion. A unique resolution key prevents applying the same
outcome twice. `wyrdstone SHARDS TOTAL-GOLD` uses the campaign's agreed arbitrary
exchange and records one sale per battle. Shards remaining in inventory retain
their identity; they are not converted to gold automatically.
Enter `wyrdstone 0 0` to record that no shards were sold; the TUI defaults to zero.

`battle injury MEMBER-ID dead` loses carried equipment and keeps the departed
member in history. `remove MEMBER-ID --status dismissed --equipment stash` returns
their gear; `--equipment lose` records its loss. Choose a successor with
`status HERO active --leader`. The player adjudicates faction-specific succession,
injury tables, spell outcomes and rare-item success; use notes/resolutions to retain
the decision. Old injury decisions are locked after their stage to avoid invalid
exploration: undo dependent changes or restore a snapshot before correcting them.

`upkeep MEMBER` pays a Hired Sword; `--waive` records an exemption and
`--depart` records departure with gear lost. The example profiles settle upkeep
in recruitment. New custom contracts can be represented by new warrior types.

Promote with `--reason 'Talent roll' promote MEMBER-ID --abilities shooting,strength`.
The original group keeps its pending advance for a reroll; the promoted Hero keeps
its ID, XP, prior increases and gear, and receives one immediate pending advance.
Use its new unit ID with `advance`. Categories or specific ability IDs/names in
`--abilities` grant its chosen access. Faction-specific choices remain the player's
responsibility and can be documented in notes.

## Random characteristics and effects

CSV type characteristics accept numbers or dice expressions such as `D6`, `2D6+1`
and `D6-1`. They remain expressions until a player resolves them:

```sh
mordheim stat MEMBER-ID S 4 --battle
mordheim --reason 'Leg wound' effect MEMBER-ID 'Leg wound' --stat M --delta -1
mordheim effect MEMBER-ID 'Mighty Blow' --stat S --delta 1 --condition 'Close combat only'
```

Battle resolutions expire outside that battle. `stat` replaces the base value
while advances and unconditional effects remain additive. Fixed-stat changes
require `--override`. Conditional effects are displayed and never indiscriminately
added to the profile. Current battle wounds are separate from the W characteristic.
`list battle` includes the frozen pre-battle reference and current battle state.

## Data, history and migration

See [docs/database.md](docs/database.md) for the table layout and editing rules.
The application writes normalized CSV tables, preserves complete prior generations,
and atomically changes the `CURRENT` pointer after syncing a complete generation.
It checks checksums, foreign keys and single-writer locking before use. A failed
command leaves campaign state unchanged. Do not edit saved generations in place.

```sh
mordheim snapshot 'Before shopping'
mordheim --reason 'Spoils' gold 20 --category reward
mordheim diff 'Before shopping'
mordheim --reason 'Wrong amount' undo
mordheim --reason 'Return to checkpoint' restore 'Before shopping'
mordheim list history
mordheim export roster.md
mordheim export ledger.csv --format ledger
mordheim export inventory.csv --format inventory
mordheim export state.json --format json
mordheim export editable --format csv
mordheim export backup.json --format bundle
mordheim --db recovered recover backup.json
```

Snapshots identify a complete database generation. **Restore and undo operate on
the whole database**, retaining original events and snapshots; use separate database
folders for independently managed campaigns. A bundle includes all retained event
states and pinned rules and is the portable backup format. State JSON/CSV is an
editable representation, not a replacement for historical backups. Exports refuse
to overwrite an existing path.

For manual reconciliation, export CSV tables to a fresh folder, edit that copy,
then preview with `import editable --reconcile`. Apply with
`--reason 'Describe changes' import editable --reconcile --apply`. History and
snapshot metadata cannot be edited. Original generations remain accessible. For
an opening-state import into an empty database, use the supplied JSON example as
a template or CSV tables without history/snapshots. Both preview before `--apply`.
Direct XLSX parsing is deferred; the supplied JSON is the documented spreadsheet
migration, including original ledger amounts and provenance notes.

To update rules, create new versioned IDs, validate/install with
`rules NEW-FOLDER --install`, then preview `migrate-rules NEW-PROFILE`.
Apply with `--reason '...' migrate-rules NEW-PROFILE --apply`. Types, equipment and
learned abilities map by unambiguous names. Missing definitions or invalid caps
block migration; pinned definitions in earlier generations remain unchanged.

## Optional AI

```sh
cargo build --locked --features ai
# Configure MORDTUI_AI_ENDPOINT (base URL ending /v1), MORDTUI_AI_MODEL,
# MORDTUI_AI_KEY and optionally MORDTUI_AI_TIMEOUT (seconds, default 60).
mordheim ai 'Suggest equipment within my budget'
# Inspect the displayed context, then opt in to submission:
mordheim ai 'Suggest equipment within my budget' --send
```

In installations with AI support, tab **7 AI assistant** provides a workbench.
Other installations show six tabs and omit AI actions and help. **N** opens a request form; sending
includes the selected warband's roster, catalogs/rules, notes, inventory, ledger,
battles and checkpoint metadata. Unrelated warbands, global event history and
credentials are excluded. Provider requests run in the background.

The assistant can propose any ordinary player command, including new equipment
and skills, purchases/equipping, battle counters, post-battle decisions, notes,
campaign rules, checkpoints and import/export/history operations. Every planned
action is listed. **R** validates the entire sequence in a disposable database and
shows actual changes and costs. **Y** opens a separate confirmation; press **Y**
there to approve and apply all reviewed actions. **Esc** cancels confirmation;
**D** rejects a proposal. Sending a request or receiving/reviewing a proposal never
applies it. You can ask follow-up questions or request a revised proposal.

Approval is single-use and expires when campaign data changes. Failed plans leave
the live database intact; successful batches publish the reviewed state together,
with ordinary validation, history and AI provenance. Exports remain staged until
approval and cannot overwrite existing files. CLI `ai` remains an advice interface.
Credentials stay in the environment and are never exported. No provider is
contacted in ordinary offline use. A local mock endpoint verifies the full
workbench request/reject/cancel/approve flow without real credentials.

## Verification and current scope

```sh
cargo test --locked
cargo test --locked --all-features
cargo clippy --locked --all-targets --all-features -- -D warnings
```

Tests cover interactive roster rendering, form keyboard/mouse routing, hiring,
purchase/allocation, XP/advances, complete post-battle forms, validation/cancellation,
warband context and narrow layouts, as well as workbook reconciliation, group
equipment conservation, advances/caps,
promotion, random stats, CSV persistence and corruption detection, locking,
resumable battles, duplicate outcomes/sales, undo/backdating, rules migration,
manual reconciliation, CLI JSON/errors and bundle restoration. The first release
uses player-assisted dice/table resolution. Complete rule libraries, automatic
exploration/rarity adjudication, complex faction automation and direct XLSX parsing
remain extensions; descriptive/manual resolutions are supported for those cases.
