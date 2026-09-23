# Fermeture du jeu — correctif du 23 septembre 2026

Le rapport Windows fourni par Dawson reproduit un panic `NoEntities` dans
`render_quick_actions` et `render_narrative` après destruction de la fenêtre.
`render_choices` avait la même hypothèse incorrecte.

Les entrées de rendu utilisent maintenant `get_single()` et s'arrêtent si la
fenêtre manque (ou si la requête est ambiguë). La même protection couvre le
rendu des menus, les confirmations et le défilement. Les accès `single()` dans
les fonctions de construction internes restent derrière ces gardes : la
requête est immuable et les commandes différées ne détruisent pas de fenêtre
au milieu de leur exécution.

Tests ajoutés : `menu_window_tests.rs`. Une application Bevy sans GPU exécute
les vrais systèmes de dialogue, choix et commandes rapides. Les tests vérifient
leur rendu normal, la destruction de la fenêtre suivie de plusieurs updates,
l'absence initiale de fenêtre et une requête contenant plusieurs fenêtres.

Vérifications effectuées sur Linux :

- Deux nouveaux tests de régression réussis (26 tests rvn_bevy au total).
- Suites release rvn_bevy, rvn_core et rvn_ui réussies.
- Construction release Linux réussie.
- Cross-compilation release Windows x86_64 GNU réussie.

Limites : ce sont des tests de systèmes Bevy, pas un retest de la croix native
sous Windows. Rejouer sur Windows la fermeture par croix et Alt+F4 pendant un
dialogue, un choix et une confirmation, depuis l'aperçu et un nouvel export.
Vérifier un arrêt sans panic et la sauvegarde/réouverture.

Les paquets 0.2.4 déjà présents sur GitHub ne contiennent pas ce correctif.
La préférence d'écriture progressive et les avertissements de traduction
signalés dans le rapport sont hors de ce correctif et restent à investiguer.
