use rvn_core::{Engine, TerminalRenderer};
use rvn_parser::parse;

fn main() {
    let script = r#"
        // Exemple d'utilisation du bloc init
        // Ce bloc s'exécute une seule fois avant le début de la narration

        init {
            // Création des personnages
            character.create("sarah", "Sarah Lemaire")
            character.create("marc", "Marc Dubois")

            // Initialisation des variables de jeu
            set score = 0
            set chapitre = 1
            set difficulte = "normale"
            set relation_sarah = 50

            // Configuration audio
            music.volume("0.8")
        }

        // Le script commence ici
        label debut
            scene "foret.png" with fade

            sarah.show() at left with dissolve
            sarah "Bienvenue dans cette aventure !"
            sarah "Ton score actuel est 0."

            choice {
                "Commencer l'aventure" => {
                    set score = 10
                    sarah "Excellent choix !"
                    jump aventure
                }
                "Voir les paramètres" => {
                    sarah "La difficulté est réglée sur 'normale'."
                    sarah "Le volume de la musique est à 80%."
                    jump debut
                }
            }

        label aventure
            marc.show("serieux") at right with dissolve
            marc "Salut ! Je vois que tu as déjà 10 points."
            sarah "Continuons ensemble !"

    "#;

    println!("=== Test du bloc init ===\n");

    let parsed = parse(script).expect("Erreur de parsing");
    println!("✓ Script parsé : {} statements", parsed.len());

    let mut engine = Engine::new(parsed, TerminalRenderer, 32).expect("Erreur d'initialisation");

    // Vérifier que les variables init sont définies
    println!("\n=== Variables après init ===");
    println!("score = {:?}", engine.get_var("score"));
    println!("chapitre = {:?}", engine.get_var("chapitre"));

    assert_eq!(engine.get_var("score"), Some(&rvn_parser::Value::Int(0)));
    assert_eq!(engine.get_var("chapitre"), Some(&rvn_parser::Value::Int(1)));

    println!("\n✓ Les variables init sont correctement initialisées");
    println!("✓ Le moteur est prêt à exécuter le script");
    println!("\n=== Test réussi ! ===");
}
