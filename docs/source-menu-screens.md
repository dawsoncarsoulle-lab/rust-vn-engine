# Écrire des menus programmables

Le Designer de menus permet d’associer explicitement une présentation à une
`screen` RVN. Les fonctions, expressions, listes, dictionnaires et boucles de
cette interface restent du code source. Le document de menus conserve son
historique d’annulation, sa sauvegarde et sa présentation de secours.

Sans association, le jeu utilise les pages de menus existantes. Une association
ne convertit jamais `menus.rvnui` en composants évalués et ne modifie pas les
interfaces d’Atlas automatiquement.

## Choisir une présentation

Le champ `source_screens` du document associe les rôles suivants aux noms des
écrans :

| Rôle | Usage et données utiles |
| --- | --- |
| `title` | Accueil, reprise disponible, origine de la navigation |
| `pause` | Partie active et commandes de navigation |
| `save`, `load` | Page, emplacements, protection et compatibilité |
| `settings` | Préférences et langues réellement disponibles |
| `gallery` | Illustrations connues, déverrouillage, sélection et fins vues |
| `history` | Lignes de dialogue vivantes |
| `confirm` | Opération en attente et jeton exact |
| `dialogue` | Personnage, texte complet, texte visible et animation |
| `choices` | Réponses visibles, indices et raccourcis |
| `quick_actions` | Actions de jeu et disponibilité du rollback |

L’écran prend zéro argument, ou un seul argument de contexte. Deux arguments
ou un écran absent produisent un diagnostic et la présentation de secours.
Les fonctions communes peuvent être partagées entre tous les écrans. Deux
rôles affichés simultanément doivent cependant avoir des écrans racines
distincts ; les rôles exclusifs peuvent réutiliser le même écran.

Le contexte contient `role`, `origin` (`title` ou `in_game`), `game_active`,
`has_save`, `can_rollback`, `settings`, `saves`, `gallery`, `history`, `dialogue`,
`choices`, `confirmation` et `diagnostic`. Il sert aux calculs de présentation.
Il n’est ni un état de partie modifiable ni une autorisation fournie par le
document. Le contexte de démonstration du Designer sert uniquement à l’aperçu.

## Relier les contrôles aux commandes

Une fonction d’écran construit des composants et reste pure. Un gestionnaire
d’événement transmet une demande au jeu avec `menu.execute`. Par exemple :

```rvn
function command_button(id, text, request, enabled) {
    return component(id, "button", {
        "text": text,
        "enabled": enabled,
        "event_data": {"request": request},
        "events": {"click": "game_command"}
    }, [])
}

handler game_command(event) {
    menu.execute(event["data"]["request"])
}

screen pause_example(context) {
    return component("pause_root", "column", {"spacing": 12}, [
        command_button("resume", "Reprendre", menu_action("resume"),
            context["game_active"]),
        command_button("save", "Sauvegarder", menu_action("save"),
            context["game_active"]),
        command_button("settings", "Réglages", menu_action("settings"), true),
        component("diagnostic", "text", {"text": context["diagnostic"]}, [])
    ])
}
```

Les constructeurs purs suivants produisent des demandes typées :

| Constructeur | Paramètres |
| --- | --- |
| `menu_action` | Action : `resume`, `save`, `load`, `settings`, `gallery`, `history`, `back`, `new_game`, `continue`, `quit`, `quick_save`, `quick_load`, `rollback`, `toggle_menu`, `toggle_skip` ou `none` |
| `menu_start_scene` | Nom d’un label existant |
| `menu_open_page` | Identifiant d’une page de menus existante |
| `menu_slot` | Opération `save`, `load` ou `delete`, puis entier d’emplacement |
| `menu_protect` | Emplacement, puis Boolean explicite |
| `menu_save_page` | Numéro de page à partir de 1 |
| `menu_number` | Clé `music_volume`, `sfx_volume`, `text_speed` ou `auto_speed`, puis nombre |
| `menu_bool` | Clé `typewriter` ou `fullscreen`, puis Boolean |
| `menu_language` | Code d’une langue disponible |
| `menu_advance`, `menu_skip_typewriter` | Aucun argument |
| `menu_choose` | Index visible à partir de 0 |
| `menu_gallery_cg`, `menu_gallery_tab` | Identifiant d’illustration déverrouillée, ou onglet `cg` / `endings` |
| `menu_confirm`, `menu_cancel` | Jeton exact du contexte de confirmation |

Les volumes sont compris entre 0 et 1 ; les vitesses entre 0,1 et 5. Les
emplacements manuels sont compris entre 1 et 1000. Une valeur Boolean ne peut
pas être remplacée par le nombre 0 ou 1. Les chemins de fichiers ne font pas
partie des demandes.

Le moteur vérifie le type, les limites, l’état réel du jeu et l’identité de
l’instance d’écran avant de valider les modifications du gestionnaire. Une
demande invalide conserve les variables, l’aléatoire et les contrôles ; elle
affiche un diagnostic local et laisse la partie utilisable. L’action valide
suivante efface ce diagnostic. La demande de rollback ne crée pas une étape
intermédiaire contenant les modifications de son propre gestionnaire.

Une page ouverte avec `menu_open_page` change la présentation, puis `back`
revient à la précédente. Cette navigation ne donne pas, à elle seule, le droit
d’écrire ou de charger un emplacement.

## Confirmation et sauvegardes

Les opérations sensibles reprennent les règles de sauvegarde existantes. Un
emplacement protégé ne peut pas être remplacé ou supprimé. Le chargement
vérifie la compatibilité de l’histoire. Un remplacement, une suppression ou
un chargement en partie peut ouvrir la confirmation du jeu.

L’écran `confirm` utilise `context["confirmation"]["token"]`. Ce jeton désigne
l’opération précise actuellement en attente. Un ancien jeton, un écran fermé
ou un autre rôle ne peut pas confirmer cette opération. Après sa consommation,
le jeton ne peut pas être rejoué. L’annulation ne réalise pas l’opération.

Les écrans de présentation du système et leur compteur de création sont
temporaires. Ils ne se retrouvent pas dans la sauvegarde narrative ou dans les
instantanés de rollback. Les préférences persistantes restent distinctes de
l’histoire et utilisent la sauvegarde des préférences du jeu.

## Fenêtres de menus et interfaces narratives

`ui.open(name, arguments, modal, layer)` appelé depuis une présentation source
ouvre un enfant de cette présentation. Cet enfant partage sa classe
d’affichage, peut recevoir les événements au-dessus de sa racine et se ferme
avec le rôle. Une couche identique et une ouverture plus récente placent
l’enfant devant la racine.

`ui.open_story(name, arguments, modal, layer)` ouvre explicitement une
interface narrative indépendante. Les arguments et les validations sont les
mêmes. Cette interface est sauvegardée, participe au rollback et survit à la
fermeture du menu qui l’a ouverte. Elle convient à l’inventaire, à la carte ou
à tout espace persistant du jeu :

```rvn
handler open_inventory(event) {
    ui.open_story("inventory", [], true, 60)
}

handler close_inventory(event) {
    ui.close("inventory")
}

screen inventory() {
    return component("inventory_root", "column", {"spacing": 12}, [
        command_button("inventory_settings", "Réglages",
            menu_action("settings"), true),
        component("inventory_close", "button", {
            "text": "Fermer",
            "events": {"click": "close_inventory"}
        }, [])
    ])
}
```

Une interface narrative réellement ouverte peut émettre des demandes de jeu
ordinaires. Son contexte ou son nom ne lui donnent aucune autorité spéciale
sur la confirmation ou les sauvegardes.

Le rejet d’une telle demande conserve la partie et apparaît dans le journal
du runtime. Les présentations source reçoivent aussi ce texte dans leur champ
`diagnostic`. Un écran narratif sans contexte ne dessine pas automatiquement
ce diagnostic : son apparence reste celle définie par l’auteur, sans bandeau
imposé au jeu.

Les modaux du système et leurs enfants sont au-dessus des couches narratives.
À l’inverse, dialogue, choix et actions rapides se placent sous une interface
narrative modale. Les indices de choix concernent uniquement les réponses
visibles : la première réponse visible conserve l’index 0 et le raccourci 1,
même si des réponses précédentes sont masquées par leurs conditions.
