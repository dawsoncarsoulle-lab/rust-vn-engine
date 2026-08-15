use std::collections::HashMap;
use std::path::{Path, PathBuf};

// ─── ERREURS ─────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub enum LocaleError {
    Io(std::io::Error),
    Parse(String),
}

impl std::fmt::Display for LocaleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "erreur disque locale : {e}"),
            Self::Parse(e) => write!(f, "erreur parsing locale : {e}"),
        }
    }
}

impl From<std::io::Error> for LocaleError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

// ─── LOCALE TABLE ─────────────────────────────────────────────────────────────

/// Table de traduction pour une langue donnée.
/// Clé   = string originale (avec éventuels [var] non résolus).
/// Valeur = string traduite  (avec éventuels [var] non résolus).
#[derive(Debug, Default, Clone)]
pub struct LocaleTable {
    pub lang: String,
    pub strings: HashMap<String, String>,
}

impl LocaleTable {
    /// Charge une table depuis un fichier TOML.
    pub fn load(lang: &str, path: &Path) -> Result<Self, LocaleError> {
        let content = std::fs::read_to_string(path)?;
        Self::parse(lang, &content)
    }

    /// Parse le contenu TOML d'un fichier de locale.
    pub fn parse(lang: &str, content: &str) -> Result<Self, LocaleError> {
        // On parse manuellement pour éviter une dépendance `toml` dans rvn_core.
        // Format attendu :
        //   [strings]
        //   "clé" = "valeur"
        //
        // Les clés et valeurs sont des strings TOML entre guillemets.
        let mut strings = HashMap::new();
        let mut in_strings = false;

        for (line_no, raw_line) in content.lines().enumerate() {
            let line = raw_line.trim();

            if line == "[strings]" {
                in_strings = true;
                continue;
            }
            if line.starts_with('[') {
                in_strings = false;
                continue;
            }

            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            if !in_strings {
                continue;
            }

            match parse_kv_line(line) {
                Some((k, v)) => {
                    strings.insert(k, v);
                }
                None => {
                    return Err(LocaleError::Parse(format!(
                        "ligne {} mal formée : {:?}",
                        line_no + 1,
                        raw_line
                    )));
                }
            }
        }

        Ok(Self {
            lang: lang.to_string(),
            strings,
        })
    }

    /// Sérialise la table en contenu TOML.
    /// Utilisé pour générer automatiquement `fr.toml` (langue par défaut).
    pub fn to_toml(&self) -> String {
        let mut out = format!(
            "# Fichier de localisation — langue : {}\n\
             # Clés = textes originaux du script.\n\
             # Valeurs = traductions. Gardez les [variables] intactes.\n\
             \n\
             [strings]\n",
            self.lang
        );
        let mut keys: Vec<&String> = self.strings.keys().collect();
        keys.sort();
        for key in keys {
            let val = &self.strings[key];
            out.push_str(&format!("{} = {}\n", toml_quote(key), toml_quote(val)));
        }
        out
    }
}

// ─── LOCALE MANAGER ───────────────────────────────────────────────────────────

/// Gestionnaire central de localisation.
/// Maintient la table de la langue courante + la table de fallback.
pub struct LocaleManager {
    /// Répertoire contenant les fichiers de locale (ex: assets/locales/).
    pub locale_dir: PathBuf,
    /// Langue par défaut (fallback si clé manquante).
    pub default_lang: String,
    /// Langue courante.
    pub current_lang: String,
    /// Langues disponibles.
    pub available_langs: Vec<String>,
    /// Table de la langue courante.
    current_table: LocaleTable,
    /// Table de fallback (langue par défaut).
    fallback_table: LocaleTable,
    /// Tables préchargées, utilisées par le runtime web sans accès disque.
    table_cache: HashMap<String, LocaleTable>,
}

impl LocaleManager {
    /// Crée un LocaleManager et charge les tables nécessaires.
    pub fn new(
        locale_dir: impl AsRef<Path>,
        default_lang: &str,
        current_lang: &str,
        available_langs: Vec<String>,
    ) -> Result<Self, LocaleError> {
        let dir = locale_dir.as_ref().to_path_buf();
        std::fs::create_dir_all(&dir)?;

        let fallback_table = Self::load_or_empty(default_lang, &dir);
        let current_table = if current_lang == default_lang {
            fallback_table.clone()
        } else {
            Self::load_or_empty(current_lang, &dir)
        };

        Ok(Self {
            locale_dir: dir,
            default_lang: default_lang.to_string(),
            current_lang: current_lang.to_string(),
            available_langs,
            current_table,
            fallback_table,
            table_cache: HashMap::new(),
        })
    }

    pub fn from_tables(
        locale_dir: impl AsRef<Path>,
        default_lang: &str,
        current_lang: &str,
        available_langs: Vec<String>,
        mut table_cache: HashMap<String, LocaleTable>,
    ) -> Self {
        let fallback_table = table_cache
            .remove(default_lang)
            .unwrap_or_else(|| LocaleTable {
                lang: default_lang.to_string(),
                strings: HashMap::new(),
            });
        let current_table = if current_lang == default_lang {
            fallback_table.clone()
        } else {
            table_cache
                .remove(current_lang)
                .unwrap_or_else(|| LocaleTable {
                    lang: current_lang.to_string(),
                    strings: HashMap::new(),
                })
        };
        table_cache.insert(default_lang.to_string(), fallback_table.clone());
        table_cache.insert(current_lang.to_string(), current_table.clone());

        Self {
            locale_dir: locale_dir.as_ref().to_path_buf(),
            default_lang: default_lang.to_string(),
            current_lang: current_lang.to_string(),
            available_langs,
            current_table,
            fallback_table,
            table_cache,
        }
    }

    /// Charge un fichier de locale ou retourne une table vide si absent.
    fn load_or_empty(lang: &str, dir: &Path) -> LocaleTable {
        let path = dir.join(format!("{lang}.toml"));
        if path.exists() {
            LocaleTable::load(lang, &path).unwrap_or_else(|e| {
                eprintln!("[locale] erreur chargement {lang}.toml : {e}");
                LocaleTable {
                    lang: lang.to_string(),
                    strings: HashMap::new(),
                }
            })
        } else {
            LocaleTable {
                lang: lang.to_string(),
                strings: HashMap::new(),
            }
        }
    }

    /// Traduit une string.
    ///
    /// Ordre de résolution :
    ///   1. Table de la langue courante
    ///   2. Table de fallback (langue par défaut)
    ///   3. La string originale elle-même
    pub fn translate<'a>(&'a self, original: &'a str) -> &'a str {
        if let Some(t) = self.current_table.strings.get(original) {
            return t.as_str();
        }
        if self.current_lang != self.default_lang {
            if let Some(t) = self.fallback_table.strings.get(original) {
                return t.as_str();
            }
        }
        original
    }

    /// Change la langue courante et recharge la table.
    pub fn set_language(&mut self, lang: &str) -> Result<(), LocaleError> {
        if lang == self.current_lang {
            return Ok(());
        }
        self.current_lang = lang.to_string();
        self.current_table = self
            .table_cache
            .get(lang)
            .cloned()
            .unwrap_or_else(|| Self::load_or_empty(lang, &self.locale_dir));
        Ok(())
    }

    /// Recharge la table de la langue courante depuis le disque.
    /// Utilisé par le hot-reload.
    pub fn reload_current(&mut self) {
        if !self.table_cache.is_empty() {
            return;
        }
        self.current_table = Self::load_or_empty(&self.current_lang.clone(), &self.locale_dir);
        if self.current_lang != self.default_lang {
            self.fallback_table = Self::load_or_empty(&self.default_lang.clone(), &self.locale_dir);
        }
    }

    /// Ajoute les strings manquantes dans le fichier de la langue par défaut.
    /// Appelé après le parsing du script pour générer/compléter `fr.toml`.
    /// Ne touche pas aux traductions existantes.
    pub fn update_default_locale(&mut self, strings: &[String]) -> Result<bool, LocaleError> {
        let mut changed = false;
        for s in strings {
            if !self.fallback_table.strings.contains_key(s.as_str()) {
                self.fallback_table.strings.insert(s.clone(), s.clone());
                changed = true;
            }
        }

        if changed {
            #[cfg(target_arch = "wasm32")]
            {
                return Ok(true);
            }
            #[cfg(not(target_arch = "wasm32"))]
            {
                let path = self.locale_dir.join(format!("{}.toml", self.default_lang));
                std::fs::write(&path, self.fallback_table.to_toml())?;
                eprintln!(
                    "[locale] {} mis à jour avec {} nouvelles clés",
                    path.display(),
                    strings.len()
                );
            }
        }

        Ok(changed)
    }

    pub fn current_lang(&self) -> &str {
        &self.current_lang
    }
    pub fn default_lang(&self) -> &str {
        &self.default_lang
    }
    pub fn locale_path(&self, lang: &str) -> PathBuf {
        self.locale_dir.join(format!("{lang}.toml"))
    }
}

// ─── COLLECTEUR DE STRINGS ────────────────────────────────────────────────────

/// Parcourt le script et collecte toutes les strings affichables
/// (dialogues + labels de choix) pour alimenter le fichier de locale.
pub fn collect_strings_from_script(script: &rvn_parser::Script) -> Vec<String> {
    let mut strings = Vec::new();
    collect_recursive(script, &mut strings);
    let mut seen = std::collections::HashSet::new();
    strings.retain(|s| seen.insert(s.clone()));
    strings
}

/// Collecte toutes les strings affichables depuis un script aplati (post-flatten_ast).
/// Après flatten_ast, tous les Statement::Dialogue et Statement::Choice sont dans
/// le tableau principal de façon linéaire — pas besoin de récursion.
/// C'est la version à utiliser sur engine.0.script.
pub fn collect_strings_from_flat_script(script: &rvn_parser::Script) -> Vec<String> {
    let mut strings = Vec::new();
    for stmt in script {
        match stmt {
            rvn_parser::Statement::Dialogue { text, .. } => {
                strings.push(text_to_locale_key(text));
            }
            rvn_parser::Statement::Choice { options } => {
                for opt in options {
                    strings.push(text_to_locale_key(&opt.label));
                }
            }
            _ => {}
        }
    }
    let mut seen = std::collections::HashSet::new();
    strings.retain(|s| seen.insert(s.clone()));
    strings
}

fn collect_recursive(stmts: &[rvn_parser::Statement], out: &mut Vec<String>) {
    use rvn_parser::Statement;
    for stmt in stmts {
        match stmt {
            Statement::Dialogue { text, .. } => {
                out.push(text_to_locale_key(text));
            }
            Statement::Choice { options } => {
                for opt in options {
                    out.push(text_to_locale_key(&opt.label));
                    collect_recursive(&opt.body, out);
                }
            }
            Statement::Use { .. } => {}
            Statement::Init { body } => collect_recursive(body, out),
            Statement::If {
                then_branch,
                else_branch,
                ..
            } => {
                collect_recursive(then_branch, out);
                collect_recursive(else_branch, out);
            }
            Statement::Imagemap { hotspots, .. } => {
                for hs in hotspots {
                    collect_recursive(&hs.body, out);
                }
            }
            _ => {}
        }
    }
}

/// Convertit un InterpolatedText en clé de locale.
/// Les expressions interpolées sont remplacées par `[varname]` si c'est
/// une simple variable, ou `[expr]` sinon.
fn expr_to_display(expr: &rvn_parser::Expr) -> String {
    use rvn_parser::{BinOpKind, Expr};
    match expr {
        Expr::Int(n) => n.to_string(),
        Expr::Float(f) => f.to_string(),
        Expr::Bool(b) => b.to_string(),
        Expr::Str(s) => s.clone(),
        Expr::Var(n) => n.clone(),
        Expr::Neg(e) => format!("-{}", expr_to_display(e)),
        Expr::Not(e) => format!("not {}", expr_to_display(e)),
        Expr::And(l, r) => format!("{} and {}", expr_to_display(l), expr_to_display(r)),
        Expr::Or(l, r) => format!("{} or {}", expr_to_display(l), expr_to_display(r)),
        Expr::BinOp { op, left, right } => {
            let op_str = match op {
                BinOpKind::Add => "+",
                BinOpKind::Sub => "-",
                BinOpKind::Mul => "*",
                BinOpKind::Div => "/",
                BinOpKind::Eq => "==",
                BinOpKind::Ne => "!=",
                BinOpKind::Lt => "<",
                BinOpKind::Le => "<=",
                BinOpKind::Gt => ">",
                BinOpKind::Ge => ">=",
            };
            format!(
                "{} {} {}",
                expr_to_display(left),
                op_str,
                expr_to_display(right)
            )
        }
        Expr::Call { name, args } => {
            let args_str: Vec<String> = args.iter().map(expr_to_display).collect();
            format!("{}({})", name, args_str.join(", "))
        }
        Expr::ListLit(items) => {
            let parts: Vec<String> = items.iter().map(expr_to_display).collect();
            format!("[{}]", parts.join(", "))
        }
        Expr::Index { target, index } => {
            format!("{}[{}]", expr_to_display(target), expr_to_display(index))
        }
    }
}

fn text_to_locale_key(text: &rvn_parser::InterpolatedText) -> String {
    use rvn_parser::TextSegment;
    text.0
        .iter()
        .map(|seg| match seg {
            TextSegment::Lit(s) => s.clone(),
            TextSegment::Interp(expr) => format!("[{}]", expr_to_display(expr)),
        })
        .collect()
}

// ─── HELPERS TOML MINIMAL ─────────────────────────────────────────────────────

/// Parse une ligne `"clé" = "valeur"` TOML.
fn parse_kv_line(line: &str) -> Option<(String, String)> {
    // Trouve le = séparateur (en dehors des strings)
    let eq_pos = find_eq(line)?;
    let key_part = line[..eq_pos].trim();
    let val_part = line[eq_pos + 1..].trim();
    let key = parse_toml_string(key_part)?;
    let val = parse_toml_string(val_part)?;
    Some((key, val))
}

fn find_eq(line: &str) -> Option<usize> {
    let mut in_str = false;
    let mut escape = false;
    for (i, c) in line.char_indices() {
        if escape {
            escape = false;
            continue;
        }
        if c == '\\' && in_str {
            escape = true;
            continue;
        }
        if c == '"' {
            in_str = !in_str;
            continue;
        }
        if c == '=' && !in_str {
            return Some(i);
        }
    }
    None
}

fn parse_toml_string(s: &str) -> Option<String> {
    let s = s.trim();
    if s.starts_with('"') && s.ends_with('"') && s.len() >= 2 {
        let inner = &s[1..s.len() - 1];
        let unescaped = inner
            .replace("\\n", "\n")
            .replace("\\t", "\t")
            .replace("\\\"", "\"")
            .replace("\\\\", "\\");
        return Some(unescaped);
    }
    if s.starts_with("\"\"\"") && s.ends_with("\"\"\"") && s.len() >= 6 {
        return Some(s[3..s.len() - 3].to_string());
    }
    None
}

fn toml_quote(s: &str) -> String {
    let escaped = s
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\t', "\\t");
    format!("\"{escaped}\"")
}

// ─── TESTS ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_TOML: &str = r#"
# Test locale
[strings]
"Bonjour !" = "Hello!"
"Tu as [score] points." = "You have [score] points."
"Option [n]" = "Option [n]"
"Texte avec \"guillemets\"" = "Text with \"quotes\""
"#;

    #[test]
    fn test_parse_locale_table() {
        let table = LocaleTable::parse("en", SAMPLE_TOML).unwrap();
        assert_eq!(table.lang, "en");
        assert_eq!(table.strings.get("Bonjour !").unwrap(), "Hello!");
        assert_eq!(
            table.strings.get("Tu as [score] points.").unwrap(),
            "You have [score] points."
        );
        assert_eq!(table.strings.len(), 4);
    }

    #[test]
    fn test_parse_escaped_quotes() {
        let table = LocaleTable::parse("en", SAMPLE_TOML).unwrap();
        assert!(table.strings.contains_key("Texte avec \"guillemets\""));
    }

    #[test]
    fn test_translate_found() {
        let dir = std::env::temp_dir().join("rvn_locale_test_found");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("en.toml"), SAMPLE_TOML).unwrap();
        std::fs::write(
            dir.join("fr.toml"),
            "[strings]\n\"Bonjour !\" = \"Bonjour !\"\n",
        )
        .unwrap();

        let mgr = LocaleManager::new(&dir, "fr", "en", vec!["fr".into(), "en".into()]).unwrap();
        assert_eq!(mgr.translate("Bonjour !"), "Hello!");
        assert_eq!(
            mgr.translate("Tu as [score] points."),
            "You have [score] points."
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_translate_fallback_to_original() {
        let dir = std::env::temp_dir().join("rvn_locale_test_fallback");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("en.toml"), "[strings]\n").unwrap();
        std::fs::write(dir.join("fr.toml"), "[strings]\n").unwrap();

        let mgr = LocaleManager::new(&dir, "fr", "en", vec!["fr".into(), "en".into()]).unwrap();
        // String absente → retourne l'original
        assert_eq!(mgr.translate("Texte non traduit"), "Texte non traduit");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_translate_fallback_chain() {
        // fr → en → original
        let dir = std::env::temp_dir().join("rvn_locale_test_chain");
        std::fs::create_dir_all(&dir).unwrap();
        // fr.toml (default) a la clé
        std::fs::write(
            dir.join("fr.toml"),
            "[strings]\n\"Bonjour\" = \"Bonjour\"\n",
        )
        .unwrap();
        // ja.toml (current) n'a pas la clé → fallback vers fr
        std::fs::write(dir.join("ja.toml"), "[strings]\n").unwrap();

        let mgr = LocaleManager::new(&dir, "fr", "ja", vec!["fr".into(), "ja".into()]).unwrap();
        assert_eq!(mgr.translate("Bonjour"), "Bonjour"); // fallback fr
        assert_eq!(mgr.translate("Inconnu"), "Inconnu"); // fallback original
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_to_toml_roundtrip() {
        let table = LocaleTable::parse("en", SAMPLE_TOML).unwrap();
        let serialized = table.to_toml();
        let reparsed = LocaleTable::parse("en", &serialized).unwrap();
        assert_eq!(table.strings, reparsed.strings);
    }

    #[test]
    fn test_update_default_locale_adds_missing() {
        let dir = std::env::temp_dir().join("rvn_locale_test_update");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("fr.toml"),
            "[strings]\n\"Existant\" = \"Existant\"\n",
        )
        .unwrap();
        std::fs::write(dir.join("en.toml"), "[strings]\n").unwrap();

        let mut mgr = LocaleManager::new(&dir, "fr", "en", vec!["fr".into(), "en".into()]).unwrap();
        let strings = vec!["Existant".to_string(), "Nouveau".to_string()];
        let changed = mgr.update_default_locale(&strings).unwrap();
        assert!(changed);

        // Vérifie que "Nouveau" a été ajouté
        let content = std::fs::read_to_string(dir.join("fr.toml")).unwrap();
        assert!(content.contains("Nouveau"));
        // "Existant" est toujours là
        assert!(content.contains("Existant"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_toml_quote_special_chars() {
        assert_eq!(toml_quote("Hello!"), "\"Hello!\"");
        assert_eq!(toml_quote("Say \"hi\""), "\"Say \\\"hi\\\"\"");
        assert_eq!(toml_quote("line\nnew"), "\"line\\nnew\"");
    }

    #[test]
    fn test_set_language() {
        let dir = std::env::temp_dir().join("rvn_locale_test_setlang");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("fr.toml"),
            "[strings]\n\"Bonjour\" = \"Bonjour\"\n",
        )
        .unwrap();
        std::fs::write(dir.join("en.toml"), "[strings]\n\"Bonjour\" = \"Hello\"\n").unwrap();

        let mut mgr = LocaleManager::new(&dir, "fr", "fr", vec!["fr".into(), "en".into()]).unwrap();
        assert_eq!(mgr.translate("Bonjour"), "Bonjour");

        mgr.set_language("en").unwrap();
        assert_eq!(mgr.translate("Bonjour"), "Hello");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
