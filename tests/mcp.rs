use clap::Parser;
use mordtui::{
    ai::{exchange, mcp, plan::Plan},
    cli::Cli,
    engine,
    model::Db,
    store::Store,
};
use serde_json::{Value, json};
use std::{
    io::{Cursor, Write},
    process::{Command, Stdio},
};

fn setup() -> (tempfile::TempDir, Store, Cli) {
    let dir = tempfile::tempdir().unwrap();
    let mut store = Store::open(dir.path().join("database")).unwrap();
    let db: Db = serde_json::from_str(include_str!("../examples/burgomeisters.json")).unwrap();
    store
        .commit(db, "Opening", "2026-10-24", "", "manual")
        .unwrap();
    let mut cli = Cli::try_parse_from(["mordheim", "mcp"]).unwrap();
    cli.db = store.root.clone();
    (dir, store, cli)
}
fn initialize(version: &str) -> Vec<Value> {
    vec![
        json!({"jsonrpc":"2.0","id":0,"method":"initialize","params":{"protocolVersion":version,"clientInfo":{"name":"test-agent","version":"1"},"capabilities":{}}}),
        json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
    ]
}
fn tool(id: u32, name: &str, arguments: Value) -> Value {
    json!({"jsonrpc":"2.0","id":id,"method":"tools/call","params":{"name":name,"arguments":arguments}})
}
fn wire(messages: &[Value]) -> Vec<u8> {
    messages
        .iter()
        .map(|m| format!("{m}\n"))
        .collect::<String>()
        .into_bytes()
}
fn serve(cli: &Cli, calls: Vec<Value>) -> Vec<Value> {
    let mut messages = initialize("2025-11-25");
    messages.extend(calls);
    let mut output = vec![];
    mcp::serve(cli, Cursor::new(wire(&messages)), &mut output).unwrap();
    String::from_utf8(output)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}
fn proposal() -> Value {
    json!({"summary":"Add funds and a note as requested","actions":[{"description":"Add ten gold crowns","args":["gold","10"],"reason":"Player agreed campaign award"},{"description":"Record the captain's note","args":["note","hans","Keep watch"],"reason":"Player requested reminder"}]})
}

#[test]
fn real_stdio_process_reads_and_submits_while_the_tui_writer_lock_is_held() {
    let (_dir, store, cli) = setup();
    let before = store.db.clone();
    let mut messages = initialize("2025-11-25");
    messages.extend([
        json!({"jsonrpc":"2.0","id":1,"method":"tools/list"}),
        tool(2, "mordtui_context", json!({})),
        tool(
            3,
            "mordtui_propose",
            json!({"generation":store.generation,"warband":"example-warband","plan":proposal()}),
        ),
    ]);
    let mut child = Command::new(env!("CARGO_BIN_EXE_mordheim"))
        .arg("--db")
        .arg(&cli.db)
        .arg("mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(&wire(&messages))
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let replies: Vec<Value> = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(replies.len(), 4);
    assert_eq!(replies[0]["result"]["protocolVersion"], "2025-11-25");
    let tools = replies[1]["result"]["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 6);
    assert!(
        !tools
            .iter()
            .any(|t| t["name"].as_str().unwrap().contains("approve"))
    );
    let context = &replies[2]["result"]["structuredContent"];
    assert_eq!(context["generation"], store.generation);
    assert_eq!(context["warband"], "example-warband");
    assert_eq!(
        context["context"]["selected_warband_and_rules"]["events"],
        json!([])
    );
    assert_eq!(
        replies[3]["result"]["structuredContent"]["changes_applied"],
        false
    );
    let id = replies[3]["result"]["structuredContent"]["id"]
        .as_str()
        .unwrap();
    assert_eq!(
        exchange::status(&store.root, id).unwrap()["decision"]["status"],
        "pending"
    );
    assert_eq!(
        Store::read_snapshot(&store.root).unwrap(),
        (store.generation.clone(), before)
    );
    assert_eq!(exchange::pending(&store.root).unwrap().len(), 1);
}

#[test]
fn cli_tasks_and_request_linkage_work_while_the_database_is_open() {
    let (_dir, store, cli) = setup();
    let result = Command::new(env!("CARGO_BIN_EXE_mordheim"))
        .arg("--db")
        .arg(&cli.db)
        .args(["--json", "ai", "Help with the captain"])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let output: Value = serde_json::from_slice(&result.stdout).unwrap();
    let request = &output["request"];
    assert_eq!(request["warband"], "example-warband");
    let replies = serve(
        &cli,
        vec![
            tool(1, "mordtui_requests", json!({})),
            tool(
                2,
                "mordtui_propose",
                json!({"generation":store.generation,"request_id":request["id"],"plan":proposal()}),
            ),
        ],
    );
    assert_eq!(
        replies[1]["result"]["structuredContent"]["requests"][0]["id"],
        request["id"]
    );
    assert_eq!(replies[2]["result"]["isError"], false);
    let id = replies[2]["result"]["structuredContent"]["id"]
        .as_str()
        .unwrap();
    assert_eq!(
        exchange::proposal(&store.root, id)
            .unwrap()
            .request_id
            .as_deref(),
        request["id"].as_str()
    );
    assert_eq!(Store::read_snapshot(&store.root).unwrap().1, store.db);
}

#[test]
fn agent_tools_cannot_approve_execute_supply_approval_or_change_database() {
    let (_dir, store, cli) = setup();
    let mut approved = proposal();
    approved["approved"] = true.into();
    let mut recursive = proposal();
    recursive["actions"][0]["args"] = json!(["mcp"]);
    let mut other_db = proposal();
    other_db["actions"][0]["args"] = json!(["--db=/tmp/other-db", "gold", "10"]);
    let responses = serve(
        &cli,
        vec![
            tool(1, "mordtui_approve", json!({"approved":true})),
            tool(2, "mordtui_execute", json!({"args":["gold","10"]})),
            tool(
                3,
                "mordtui_propose",
                json!({"generation":store.generation,"plan":approved}),
            ),
            tool(
                4,
                "mordtui_propose",
                json!({"generation":store.generation,"plan":recursive}),
            ),
            tool(
                5,
                "mordtui_propose",
                json!({"generation":store.generation,"plan":other_db}),
            ),
            tool(
                6,
                "mordtui_propose",
                json!({"generation":store.generation,"plan":proposal(),"approved":true}),
            ),
            tool(7, "mordtui_proposal_status", json!({"id":"../../CURRENT"})),
            tool(8, "mordtui_context", json!({"warband":"Missing warband"})),
        ],
    );
    for response in responses.iter().skip(1) {
        assert_eq!(response["result"]["isError"], true, "{response}");
    }
    assert!(exchange::pending(&store.root).unwrap().is_empty());
    assert_eq!(Store::read_snapshot(&store.root).unwrap().1, store.db);
}

#[test]
fn stale_proposals_and_invalid_batches_leave_state_untouched_and_report_feedback() {
    let (_dir, mut store, cli) = setup();
    let generation = store.generation.clone();
    let plan = Plan::parse(&proposal().to_string()).unwrap();
    let proposed = exchange::submit(
        &store.root,
        &generation,
        Some("example-warband"),
        None,
        plan,
    )
    .unwrap();
    engine::execute(
        &mut store,
        &Cli::try_parse_from(["mordheim", "gold", "1"]).unwrap(),
    )
    .unwrap();
    let before = store.db.clone();
    let responses = serve(
        &cli,
        vec![
            tool(
                1,
                "mordtui_propose",
                json!({"generation":generation,"plan":proposal()}),
            ),
            tool(2, "mordtui_proposal_status", json!({"id":proposed.id})),
        ],
    );
    assert_eq!(responses[1]["result"]["isError"], true);
    assert_eq!(
        responses[2]["result"]["structuredContent"]["decision"]["status"],
        "stale"
    );
    let invalid = Plan::parse(r#"{"summary":"Invalid batch","actions":[{"description":"Add gold","args":["gold","100"]},{"description":"Buy unknown item","args":["buy","Missing item","1"]}]}"#).unwrap();
    let offered = exchange::submit(
        &store.root,
        &store.generation,
        Some("example-warband"),
        None,
        invalid,
    )
    .unwrap();
    let error = mordtui::ai::plan::review(&store, offered.warband.as_deref(), &offered.plan)
        .err()
        .unwrap();
    exchange::record_review(&store.root, &offered, false, &error.to_string()).unwrap();
    let status = exchange::status(&store.root, &offered.id).unwrap();
    assert_eq!(status["review"]["valid"], false);
    assert!(
        status["review"]["report"]
            .as_str()
            .unwrap()
            .contains("Action 2 failed")
    );
    assert_eq!(Store::read_snapshot(&store.root).unwrap().1, before);
}

#[test]
fn requests_schemas_and_unused_rules_are_discoverable_without_network_or_a_warband() {
    let dir = tempfile::tempdir().unwrap();
    let mut cli = Cli::try_parse_from(["mordheim", "mcp"]).unwrap();
    cli.db = dir.path().join("empty");
    let request = exchange::request(&cli.db, "", None, "Design a custom warband").unwrap();
    let responses = serve(
        &cli,
        vec![
            tool(1, "mordtui_context", json!({})),
            tool(2, "mordtui_commands", json!({})),
            tool(3, "mordtui_requests", json!({})),
            tool(4, "mordtui_rules", json!({})),
            tool(
                5,
                "mordtui_propose",
                json!({"generation":"","request_id":request.id,"plan":{"summary":"Define the rules in stages","actions":[]}}),
            ),
        ],
    );
    assert_eq!(
        responses[1]["result"]["structuredContent"]["generation"],
        ""
    );
    assert!(responses[1]["result"]["structuredContent"]["warband"].is_null());
    let schema = &responses[2]["result"]["structuredContent"];
    assert_eq!(schema["plan_schema"]["additionalProperties"], false);
    assert!(
        schema["commands"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["path"] == json!(["rulebook", "save"]))
    );
    assert_eq!(
        responses[3]["result"]["structuredContent"]["requests"][0]["id"],
        request.id
    );
    assert_eq!(
        responses[4]["result"]["structuredContent"]["profiles"],
        json!([])
    );
    assert_eq!(responses[5]["result"]["isError"], false);
    assert!(!cli.db.join("CURRENT").exists());
    // The catalog endpoint also works for profiles that have no recruited warband.
    let mut store = Store::open(&cli.db).unwrap();
    let mut db: Db = serde_json::from_str(include_str!("../examples/burgomeisters.json")).unwrap();
    let profile_id = db.profiles[0].id.clone();
    db.warbands.clear();
    db.units.clear();
    db.members.clear();
    db.allocations.clear();
    db.holdings.clear();
    db.ledger.clear();
    db.advances.clear();
    db.notes.clear();
    store
        .commit(db, "Unused profile", "2026-10-24", "", "manual")
        .unwrap();
    let responses = serve(
        &cli,
        vec![tool(1, "mordtui_rules", json!({"profile":profile_id}))],
    );
    assert_eq!(responses[1]["result"]["isError"], false, "{}", responses[1]);
    assert!(
        !responses[1]["result"]["structuredContent"]["types"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn protocol_handles_handshakes_notifications_and_errors_without_stdout_noise() {
    let (_dir, _store, cli) = setup();
    let mut messages = vec![
        json!({"jsonrpc":"2.0","id":99,"method":"server/discover"}),
        json!({"jsonrpc":"2.0","id":1,"method":"tools/list"}),
    ];
    messages.extend(initialize("2025-03-26"));
    messages.extend([
        json!({"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":123}}),
        json!({"jsonrpc":"2.0","id":2,"method":"ping"}),
        json!({"jsonrpc":"2.0","id":3,"method":"unknown"}),
        json!({"jsonrpc":"2.0","id":4,"method":"tools/call","params":{}}),
    ]);
    let mut input = wire(&messages);
    input.extend_from_slice(b"{bad json}\n[]\n");
    let mut output = vec![];
    mcp::serve(&cli, Cursor::new(input), &mut output).unwrap();
    let responses: Vec<Value> = String::from_utf8(output)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(responses.len(), 8);
    assert_eq!(responses[0]["error"]["code"], -32601);
    let responses = &responses[1..];
    assert_eq!(responses[0]["error"]["code"], -32002);
    assert_eq!(responses[1]["result"]["protocolVersion"], "2025-03-26");
    assert_eq!(responses[2]["result"], json!({}));
    assert_eq!(responses[3]["error"]["code"], -32601);
    assert_eq!(responses[4]["error"]["code"], -32602);
    assert_eq!(responses[5]["error"]["code"], -32700);
    assert_eq!(responses[6]["error"]["code"], -32600);
}
