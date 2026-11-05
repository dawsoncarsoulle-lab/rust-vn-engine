use crate::types::GameState;
use rvn_parser::Hotspot;
#[path="rollback_audio.rs"]
mod captured_audio;

// ─── ENTRÉE D'HISTORIQUE ─────────────────────────────────────────────────────

/// Ce qui était affiché juste avant le snapshot — permet de re-afficher
/// le contenu lors d'un rollback sans re-exécuter le statement.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum HistoryDisplay {
    Dialogue {
        character: Option<String>,
        text: String,
    },
    Choice {
        options: Vec<String>,
    },
    Imagemap {
        background: String,
        hover_image: Option<String>,
        hotspots: Vec<Hotspot>,
    },
}

#[derive(Clone, Debug)]
pub struct HistoryEntry {
    pub state: GameState,
    pub display: Option<HistoryDisplay>,
    pub(crate) audio_publication_pending:bool,
    pub(crate) audio_publication_anchor:Option<GameState>,
    /// Process-local visit identity, shared by text and audio feedback; never saved.
    pub(crate) entry_id: Option<u64>,
}

// ─── PILE DE ROLLBACK ────────────────────────────────────────────────────────

/// Pile bornée de snapshots.
///
/// Invariant : `entries[i].state` est l'état *juste avant* le i-ème
/// statement interactif. Quand on dépile, on restaure cet état et on
/// réaffiche le contenu associé.
pub struct RollbackHistory {
    entries: Vec<HistoryEntry>,
    max_size: usize,
    next_entry_id: Option<u64>,
}

impl RollbackHistory {
    pub(crate) fn debugger_copy(&self)->Self {
        Self{entries:self.entries.clone(),max_size:self.max_size,next_entry_id:self.next_entry_id}
    }
    pub(crate) fn debugger_write(&self,out:&mut impl std::fmt::Write)->std::fmt::Result {
        write!(out,"{:?}{:?}{:?}",self.entries,self.max_size,self.next_entry_id)
    }
    pub fn new(max_size: usize) -> Self {
        Self {
            entries: Vec::new(),
            max_size,
            next_entry_id: Some(0),
        }
    }

    pub fn push(&mut self, mut state: GameState, display: Option<HistoryDisplay>) {
        state.ui = state.ui.story_only();
        if self.entries.len() >= self.max_size {
            self.entries.remove(0);
        }
        let entry_id = self.next_entry_id.and_then(|id| id.checked_add(1));
        self.next_entry_id = entry_id;
        self.entries.push(HistoryEntry { state, display, entry_id, audio_publication_pending:false, audio_publication_anchor:None });
    }

    pub(crate) fn latest_entry_id(&self) -> Option<u64> {
        self.entries.last().and_then(|entry| entry.entry_id)
    }

    /// Only the latest exact display snapshot receives observed presenter metrics.
    /// A recursive/repeated position must never update an older visit by alias.
    pub(crate) fn record_display_reveal(&mut self, entry_id: Option<u64>, position: &crate::scene::ResumePosition,
        owner: Option<crate::scene::ActivationId>, reveal: &crate::display_reveal::RevealCheckpoint) {
        let Some(entry_id) = entry_id else { return; };
        let Some(entry) = self.entries.last_mut() else { return; };
        if entry.entry_id != Some(entry_id) { return; }
        let Some(execution) = &mut entry.state.scene_execution else { return; };
        if execution.owner() != owner { return; }
        let Some(display) = &mut execution.display else { return; };
        if display.position == *position && matches!(display.display, HistoryDisplay::Dialogue { .. }) {
            display.reveal = Some(reveal.clone());
        }
    }

    pub fn pop(&mut self) -> Option<HistoryEntry> {
        self.entries.pop()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn can_rollback(&self) -> bool {
        !self.entries.is_empty()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Accès en lecture pour les tests.
    pub fn entries(&self) -> &[HistoryEntry] {
        &self.entries
    }
}

#[cfg(test)]
mod exact_history_identity_tests {
    use super::*;
    #[test]
    fn history_visit_ids_survive_trim_clear_and_refuse_reuse_after_checked_exhaustion() {
        let state=crate::Engine::new(rvn_parser::parse("").unwrap(),crate::renderer::TerminalRenderer,8).unwrap().state;
        let mut history=RollbackHistory::new(1);
        history.push(state.clone(),None);let first=history.latest_entry_id().unwrap();
        history.push(state.clone(),None);let second=history.latest_entry_id().unwrap();
        assert_eq!(history.len(),1);assert_ne!(second,first);
        history.clear();history.push(state.clone(),None);let third=history.latest_entry_id().unwrap();
        assert!(third>second);
        history.next_entry_id=Some(u64::MAX);
        history.push(state.clone(),None);assert_eq!(history.latest_entry_id(),None);assert_eq!(history.next_entry_id,None);
        history.clear();history.push(state,None);assert_eq!(history.latest_entry_id(),None);assert_eq!(history.next_entry_id,None);
        assert_eq!(history.len(),1);assert_eq!(history.entries[0].entry_id,None);
    }
}
