//! The injury report includes unresolved casualties, resolved outcomes and lasting effects.
use super::*;

pub(super) struct InjuryEntry {
    pub id: String,
    pub member: String,
    pub cells: Vec<String>,
}
impl App {
    pub(super) fn injuries(&self, db: &Db) -> Vec<InjuryEntry> {
        let Some(wb) = &self.wb else {
            return vec![];
        };
        let mut rows = vec![];
        for m in db.members_of(wb) {
            let status = if m.missed_games > 0 {
                format!("{}; misses {} game(s)", m.status, m.missed_games)
            } else {
                m.status.clone()
            };
            let outcomes: Vec<_> = db
                .resolutions
                .iter()
                .filter(|r| r.key == format!("injury-{}", m.id))
                .collect();
            let effects: Vec<_> = db.effects.iter().filter(|e| e.member_id == m.id).collect();
            for p in db.participants.iter().filter(|p| {
                p.member_id == m.id
                    && p.condition == "out-of-action"
                    && !p.injury_resolved
                    && db
                        .battles
                        .iter()
                        .any(|b| b.id == p.battle_id && b.status != "abandoned")
            }) {
                let b = db.battles.iter().find(|b| b.id == p.battle_id).unwrap();
                rows.push(InjuryEntry {
                    id: p.id.clone(),
                    member: m.id.clone(),
                    cells: vec![
                        b.date.clone(),
                        m.name.clone(),
                        format!("UNRESOLVED · {}", b.name),
                        "—".into(),
                        status.clone(),
                    ],
                });
            }
            for r in &outcomes {
                let b = db.battles.iter().find(|b| b.id == r.battle_id).unwrap();
                let changes = effects
                    .iter()
                    .filter(|e| e.description == r.description)
                    .map(|e| effect_change(e))
                    .collect::<Vec<_>>()
                    .join(", ");
                rows.push(InjuryEntry {
                    id: r.id.clone(),
                    member: m.id.clone(),
                    cells: vec![
                        b.date.clone(),
                        m.name.clone(),
                        format!("{} · {}", r.description, b.name),
                        changes,
                        status.clone(),
                    ],
                });
            }
            for e in effects
                .iter()
                .filter(|e| !outcomes.iter().any(|r| r.description == e.description))
            {
                rows.push(InjuryEntry {
                    id: e.id.clone(),
                    member: m.id.clone(),
                    cells: vec![
                        "Lasting".into(),
                        m.name.clone(),
                        e.description.clone(),
                        effect_change(e),
                        status.clone(),
                    ],
                });
            }
            if outcomes.is_empty()
                && effects.is_empty()
                && matches!(m.status.as_str(), "recovering" | "captured")
            {
                rows.push(InjuryEntry {
                    id: m.id.clone(),
                    member: m.id.clone(),
                    cells: vec![
                        "Current".into(),
                        m.name.clone(),
                        "Recovery / captivity".into(),
                        "—".into(),
                        status,
                    ],
                });
            }
        }
        rows.retain(|r| {
            r.cells
                .join(" ")
                .to_lowercase()
                .contains(&self.filter.to_lowercase())
        });
        rows
    }
}
fn effect_change(e: &Effect) -> String {
    let value = if e.characteristic.is_empty() {
        "Descriptive".into()
    } else {
        format!("{}{:+}", e.characteristic, e.delta)
    };
    if e.condition.is_empty() {
        value
    } else {
        format!("{value} ({})", e.condition)
    }
}
