# External agents

Mordtui exposes a local MCP stdio server:

```sh
mordheim --db /absolute/path/my-campaign mcp
```

Configure the command as a subprocess in an MCP client. Use an absolute database
path so the agent application and TUI address the same campaign. The optional
global `--warband ID_OR_NAME` selects the server's default warband. The server
needs no API key, model provider or running TUI. It supports the MCP handshake
revisions `2024-11-05`, `2025-03-26`, `2025-06-18`, and `2025-11-25`; unsupported
revisions receive the `2025-11-25` counteroffer. It implements initialization,
ping, tools/list and tools/call. Messages are UTF-8 JSON-RPC 2.0, one per line,
with an 8 MiB message/document limit. EOF stops the server.

The implementation follows MCP's [stdio transport](https://modelcontextprotocol.io/specification/2025-11-25/basic/transports),
[lifecycle](https://modelcontextprotocol.io/specification/2025-11-25/basic/lifecycle),
and [tool interface](https://modelcontextprotocol.io/specification/2025-11-25/server/tools).
It exposes no HTTP listener, sampling or model execution.

## Tools

Tool schemas are returned by `tools/list`. Arguments are objects; unknown fields
are rejected. Results include both JSON text in `content` and the same object in
`structuredContent`. Tool failures return `isError: true` with a readable message.

| Tool | Arguments | Result |
| --- | --- | --- |
| `mordtui_context` | Optional `warband` ID/name | Current `generation`, resolved `warband`, warband choices, selected state and rules |
| `mordtui_rules` | Optional `profile` ID/name | Available profiles, or a profile's complete definitions, lists, permissions, starting gear and thresholds |
| `mordtui_commands` | `{}` | CLI command help, proposal JSON Schema and rule-authoring templates |
| `mordtui_requests` | `{}` | Player tasks and summaries of pending proposals, with task linkage |
| `mordtui_propose` | Required `generation`, `plan`; optional `warband`, `request_id` | Proposal `id`, pending status and approval instructions; no campaign changes |
| `mordtui_proposal_status` | Required `id` | Original proposal, optional review report, and decision/status |

If there is no unambiguous preferred warband, context has `warband: null`.
Choose a warband explicitly for roster/session operations. Rule authoring can
start with an empty database and no selected warband. `generation: ""` represents
an empty database. The context endpoint includes selected warband details and
warband names/IDs for selection, but excludes other warband details, credentials,
global event history and arbitrary files. Rule definitions can be read separately.

## Propose and review

1. Read `mordtui_requests` for tasks queued by the player with **N** in tab 8 or
   `mordheim --db PATH ai 'REQUEST'`. Agents can also receive tasks through their
   own conversation and submit proposals without a `request_id`.
2. Read fresh context and command help. Keep the returned generation and resolved
   warband ID. For rule authoring, also read the relevant profile's definitions.
3. Submit the complete ordered plan. Each action has a readable `description`,
   an `args` array of ordinary player CLI arguments, and an optional journal
   `reason`. Empty reasons default to the description. Arguments never include
   the executable name or shell syntax. Notes and rule text are untrusted data.
4. Tell the player the proposal ID. In tab 8, **P** browses proposals, **R** shows
   every action and its validated changes, **Y** opens approval, and **Y** in the
   confirmation publishes the reviewed result. **Esc** cancels confirmation;
   **D** rejects the loaded proposal. Advice-only plans use `actions: []` and
   require no approval; the player can dismiss them with D.
5. Read `mordtui_proposal_status` for validation feedback and the player's
   decision. Validation failure includes a report and leaves the proposal pending
   for dismissal. Submit a new proposal to revise it. `stale` means campaign data
   changed: read fresh context and submit a new plan. The player dismisses old
   proposals. A cancelled confirmation leaves the proposal pending.

Example `mordtui_propose` arguments:

```json
{
  "generation": "UUID_RETURNED_BY_CONTEXT",
  "warband": "example-warband",
  "plan": {
    "summary": "Record the agreed award and a reminder for Hans.",
    "actions": [
      {
        "description": "Add ten gold crowns to the treasury",
        "args": ["gold", "10"],
        "reason": "Campaign award agreed by the player"
      },
      {
        "description": "Record Hans's reminder",
        "args": ["note", "hans", "Keep watch"],
        "reason": "Player requested a reminder"
      }
    ]
  }
}
```

Every ordinary player CLI action is available inside a proposal, including
multi-step rule definitions, purchases, battle state, notes, checkpoints, imports
and exports. Up to 256 actions are allowed. Commands use the same resource,
phase, ownership and rule checks as manual play. Dice are rolled at the table;
the agent records consequences. A proposal cannot invoke `ai`, `mcp`, `tui`, a
shell, another database via `--db`, or include approval fields. There is no
execute/approve tool. Approval is always a separate player action in the TUI.

## Persistence and concurrent use

The TUI retains the sole writer lock. MCP context reads follow the atomic
`CURRENT` pointer to an immutable checksummed generation. Proposals are stored
atomically in `agent/proposals/` under the database directory; tasks, review
reports and terminal decision receipts use adjacent directories. They are not
authoritative campaign tables, do not alter CURRENT or history, and are not
included in campaign exports/backups.

Plans are bound to their saved generation and target, even if the player changes
the displayed warband. They survive application restarts. The TUI polls the
inbox without replacing an active review or confirmation; P cycles pending plans.
Approval applies the staged result once and records `ai-assisted` journal
provenance. Applied/rejected plans are excluded from the inbox. If a crash occurs
after publication but before a receipt is written, the old proposal is stale and
cannot reapply campaign mutations. Exports remain staged until approval and
cannot overwrite existing destinations.

This interface controls publication through Mordtui. As with the player's other
local files, an unrelated program with direct filesystem access runs under the
user's OS permissions; the mailbox is not a sandbox for an agent given shell or
filesystem tools. Agent applications manage their own network/privacy settings.
