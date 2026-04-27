use crate::types::GameState;
use rvn_parser::Hotspot;

// ─── ENTRÉE D'HISTORIQUE ─────────────────────────────────────────────────────

/// Ce qui était affiché juste avant le snapshot — permet de re-afficher
/// le contenu lors d'un rollback sans re-exécuter le statement.
#[derive(Clone, Debug)]
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
}

impl RollbackHistory {
    pub fn new(max_size: usize) -> Self {
        Self {
            entries: Vec::new(),
            max_size,
        }
    }

    pub fn push(&mut self, state: GameState, display: Option<HistoryDisplay>) {
        if self.entries.len() >= self.max_size {
            self.entries.remove(0);
        }
        self.entries.push(HistoryEntry { state, display });
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
