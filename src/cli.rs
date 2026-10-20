use clap::{Parser, Subcommand};
use std::path::PathBuf;
#[derive(Parser, Debug)]
#[command(
    name = "mordheim",
    version,
    about = "Offline Mordheim campaign manager. Run without a command for the TUI.",
    after_help = "Examples:\n  mordheim init --rules examples/rules --profile reikland --campaign 'City of the Damned' --name 'The Watch'\n  mordheim list heros\n  mordheim recruit Captain Hans\n  mordheim --reason 'Campaign agreement' --override wyrdstone 2 80\nUse --help on any command. Run tui for roster tables and session forms; : is an optional advanced prompt."
)]
pub struct Cli {
    #[arg(
        long,
        global = true,
        default_value = "mordheim-db",
        help = "CSV database folder"
    )]
    pub db: PathBuf,
    #[arg(long, global = true, help = "Campaign ID or unambiguous name")]
    pub campaign: Option<String>,
    #[arg(long, global = true, help = "Warband ID or unambiguous name")]
    pub warband: Option<String>,
    #[arg(
        long,
        global = true,
        help = "Effective date YYYY-MM-DD; defaults to today"
    )]
    pub date: Option<String>,
    #[arg(
        long,
        global = true,
        default_value = "",
        help = "Audit justification; required for overrides"
    )]
    pub reason: String,
    #[arg(
        long = "override",
        global = true,
        help = "Accept a rules exception with --reason; structural invariants still apply"
    )]
    pub override_rules: bool,
    #[arg(long, global = true, help = "Machine-readable output")]
    pub json: bool,
    #[command(subcommand)]
    pub command: Option<Command>,
}
#[derive(Subcommand, Debug)]
pub enum Command {
    /// Create a campaign and warband, pin rules, and record starting funding
    Init {
        #[arg(long)]
        rules: PathBuf,
        #[arg(long)]
        profile: String,
        #[arg(long)]
        campaign: String,
        #[arg(long)]
        name: String,
    },
    Campaign {
        name: String,
    },
    /// Set a persistent rule for the selected warband's entire campaign
    CampaignRules {
        #[arg(long, action = clap::ArgAction::Set)]
        skip_dramatis_personae: bool,
    },
    Warband {
        name: String,
        #[arg(long)]
        profile: String,
    },
    /// Remember a warband for subsequent CLI invocations
    Select {
        warband: String,
    },
    /// List summary, campaigns, warbands, heroes (heros), henchmen, roster, equipment, ledger, battles, history, snapshots, notes, rules, or dice
    List {
        #[arg(default_value = "summary")]
        what: String,
        #[arg(long)]
        filter: Option<String>,
    },
    Show {
        entity: String,
    },
    Rules {
        folder: PathBuf,
        #[arg(long)]
        install: bool,
    },
    /// Extend the selected profile's equipment catalog without editing rule files
    AddEquipment {
        name: String,
        #[arg(long, default_value_t = 0)]
        cost: i64,
        #[arg(long, default_value = "melee")]
        category: String,
        #[arg(long, default_value_t = 0)]
        rarity: u32,
        #[arg(long, default_value = "")]
        description: String,
        /// Equipment lists to receive the item; defaults to all lists in this profile
        #[arg(long = "list", value_delimiter = ',')]
        lists: Vec<String>,
        #[arg(long, value_delimiter = ',', hide = true)]
        allowed: Vec<String>,
    },
    /// Create shared equipment lists, assign them, or edit an item's lists
    #[command(subcommand)]
    EquipmentList(EquipmentListCommand),
    /// Define a skill, spell or prayer and its eligible warriors in the TUI/CLI
    AddAbility {
        name: String,
        #[arg(long, default_value = "combat")]
        kind: String,
        #[arg(long, default_value = "")]
        description: String,
        #[arg(long, value_delimiter = ',', default_value = "heroes")]
        allowed: Vec<String>,
    },
    /// Record an existing skill without spending a pending XP advance
    GrantAbility {
        unit: String,
        ability: String,
    },
    /// Hire a Hero, Hired Sword, or a group; add to an existing group with --group
    Recruit {
        kind: String,
        name: String,
        #[arg(long, default_value_t = 1)]
        count: u32,
        #[arg(long)]
        group: Option<String>,
        #[arg(long)]
        price: Option<i64>,
    },
    Remove {
        member: String,
        #[arg(long, default_value = "dismissed")]
        status: String,
        #[arg(long, default_value = "stash")]
        equipment: String,
    },
    Rename {
        entity: String,
        name: String,
    },
    Xp {
        unit: String,
        amount: u32,
    },
    /// Resolve the next pending advance; use --initial for a paid setup advance
    Advance {
        unit: String,
        #[arg(long)]
        stat: Option<String>,
        #[arg(long)]
        ability: Option<String>,
        #[arg(long)]
        dice: Option<String>,
        #[arg(long)]
        initial: bool,
    },
    Promote {
        member: String,
        #[arg(long, value_delimiter = ',')]
        abilities: Vec<String>,
        #[arg(long)]
        dice: Option<String>,
    },
    /// Add a lasting or contextual characteristic effect / descriptive injury
    Effect {
        member: String,
        description: String,
        #[arg(long)]
        stat: Option<String>,
        #[arg(long, default_value_t = 0, allow_hyphen_values = true)]
        delta: i16,
        #[arg(long, default_value = "")]
        condition: String,
        #[arg(long)]
        battle: Option<String>,
    },
    /// Resolve a random characteristic (or override a fixed one) with a recorded reason
    Stat {
        member: String,
        characteristic: String,
        value: i16,
        #[arg(long)]
        battle: bool,
        #[arg(long)]
        dice: Option<String>,
    },
    Status {
        member: String,
        status: String,
        #[arg(long, default_value_t = 0)]
        missed: u32,
        #[arg(long)]
        leader: bool,
    },
    Buy {
        item: String,
        quantity: u32,
        #[arg(long)]
        price: Option<i64>,
    },
    Sell {
        item: String,
        quantity: u32,
        #[arg(long)]
        price: i64,
    },
    Find {
        item: String,
        quantity: u32,
    },
    Lose {
        item: String,
        quantity: u32,
    },
    Equip {
        unit: String,
        item: String,
        #[arg(default_value_t = 1)]
        quantity: u32,
    },
    Unequip {
        unit: String,
        item: String,
    },
    /// Signed treasury adjustment (funding, rewards, expenses or correction)
    Gold {
        #[arg(allow_hyphen_values = true)]
        amount: i64,
        #[arg(long, default_value = "adjustment")]
        category: String,
    },
    /// Exchange shards for the total gold amount agreed in your campaign
    Wyrdstone {
        shards: u32,
        gold: i64,
    },
    Note {
        entity: String,
        text: String,
        #[arg(long)]
        replace: Option<String>,
    },
    /// Record dice; --results accepts physical rolls, otherwise the tool rolls
    Roll {
        notation: String,
        #[arg(long, value_delimiter = ',')]
        results: Vec<u32>,
        #[arg(long, default_value = "")]
        rerolls: String,
        #[arg(long, default_value = "manual")]
        table: String,
        #[arg(long, default_value = "")]
        choice: String,
    },
    #[command(subcommand)]
    Battle(BattleCommand),
    /// Legacy search allowance bookkeeping; use rare-search for item-specific trading
    Search {
        member: String,
        kind: String,
        #[arg(long)]
        allowance: Option<u32>,
        #[arg(long)]
        use_attempt: bool,
    },
    /// Record the consequence of one eligible Hero's physical rare-item search
    RareSearch {
        member: String,
        item: String,
        #[arg(long, action = clap::ArgAction::Set)]
        found: bool,
    },
    /// Record veteran availability after rolling at the table
    Veterans {
        experience: u32,
    },
    /// Record a keyed manual outcome once, optionally granting resources
    Resolve {
        key: String,
        description: String,
        #[arg(long, default_value_t = 0, allow_hyphen_values = true)]
        gold: i64,
        #[arg(long, default_value_t = 0)]
        shards: u32,
        #[arg(long)]
        dice: Option<String>,
    },
    /// Complete or skip the current ordered post-battle stage
    Step {
        name: String,
        #[arg(long, conflicts_with = "begin")]
        skip: bool,
        #[arg(long)]
        begin: bool,
    },
    Upkeep {
        member: String,
        #[arg(long)]
        waive: bool,
        #[arg(long)]
        depart: bool,
    },
    Snapshot {
        name: String,
    },
    Diff {
        left: String,
        right: Option<String>,
    },
    /// Restore a named snapshot as a new audited commit
    Restore {
        snapshot: String,
    },
    /// Append an audited restoration of the preceding transaction
    Undo,
    /// JSON state or CSV-table opening-state import. Preview unless --apply
    Import {
        file: PathBuf,
        #[arg(long)]
        apply: bool,
        /// Reconcile a deliberately edited CSV/JSON copy into existing state
        #[arg(long)]
        reconcile: bool,
    },
    /// Export markdown, ledger/inventory CSV, state JSON or full portable bundle
    Export {
        file: PathBuf,
        #[arg(long, default_value = "markdown")]
        format: String,
    },
    /// Restore a portable bundle into an empty database
    Recover {
        file: PathBuf,
    },
    Validate,
    /// Preview pinned rules migration (commit with --apply and --reason)
    MigrateRules {
        profile: String,
        #[arg(long)]
        apply: bool,
    },
    /// Optional AI advice. --send explicitly permits submitting selected warband context
    Ai {
        prompt: String,
        #[arg(long)]
        send: bool,
    },
    Tui,
}
#[derive(Subcommand, Debug)]
pub enum EquipmentListCommand {
    Create {
        name: String,
        #[arg(long, default_value = "")]
        description: String,
        #[arg(long = "type", value_delimiter = ',')]
        types: Vec<String>,
        #[arg(long = "item", value_delimiter = ',')]
        items: Vec<String>,
    },
    /// Assign to types, units or individuals; use "inherit" to clear a model/group override
    Assign {
        list: String,
        #[arg(required = true, num_args = 1..)]
        targets: Vec<String>,
    },
    /// Replace this item's memberships; omit --list to remove it from every list
    Item {
        item: String,
        #[arg(long = "list", value_delimiter = ',')]
        lists: Vec<String>,
    },
}
#[derive(Subcommand, Debug)]
pub enum BattleCommand {
    Prepare {
        name: String,
        #[arg(long, default_value = "Skirmish")]
        scenario: String,
        #[arg(long, default_value = "Unknown")]
        opponent: String,
        #[arg(long, default_value_t = 0)]
        rating: u32,
    },
    Start,
    Record {
        member: String,
        #[arg(long)]
        condition: Option<String>,
        #[arg(long)]
        wounds: Option<i16>,
        #[arg(long)]
        kills: Option<u32>,
        #[arg(long)]
        objectives: Option<u32>,
        #[arg(long, default_value = "", hide = true)]
        attribution: String,
    },
    /// Credit one kill / OOA, optionally naming the opponent, or remove a recorded credit
    Kill {
        member: String,
        #[arg(long, default_value = "")]
        victim: String,
        /// Kill record ID, or "unspecified" for an older unnamed credit
        #[arg(long)]
        remove: Option<String>,
    },
    /// Consume an equipped item while playing
    Consume {
        member: String,
        item: String,
        #[arg(default_value_t = 1)]
        quantity: u32,
    },
    End {
        result: String,
    },
    /// Resolve an out-of-action member; death loses gear, recovery retains it
    Injury {
        member: String,
        outcome: String,
        #[arg(long)]
        dice: Option<String>,
        #[arg(long)]
        stat: Option<String>,
        #[arg(long, default_value_t = 0, allow_hyphen_values = true)]
        delta: i16,
        #[arg(long, default_value_t = 0)]
        missed: u32,
    },
    Complete,
    Abandon {
        disposition: String,
    },
    /// Show manual XP award suggestions without committing them
    Awards,
}
