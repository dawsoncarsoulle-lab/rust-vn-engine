# Afficher et retirer les personnages

Le sprite est un état de la scène, indépendant de la prise de parole.

- Un dialogue avec une image reliée au pin **Sprite** du personnage affiche cette image ou remplace son sprite actuel, sans créer un deuxième personnage.
- Un dialogue sans image reliée ne modifie pas l'affichage : le personnage reste visible s'il était déjà présent, sinon il parle hors champ.
- **Retirer le sprite** retire uniquement la représentation du personnage ciblé. Reliez le personnage à son entrée et placez ce nœud sur le fil d'exécution. Une transition est facultative. La recherche accepte aussi **Destroy Actor** et **Masquer**.
- Retirer un personnage déjà absent ne fait rien. Une image branchée lors d'un dialogue ultérieur peut le faire réapparaître.

Le retrait ne supprime ni le personnage, ni ses fichiers, ni les variables de l'histoire. Les règles existantes de changement de scène restent inchangées.

## Compatibilité

Les graphes existants restent lisibles : le nœud conserve son type `SpriteHide`. Aucun graphe de l'histoire n'est réécrit automatiquement. Si un ancien passage comptait sur un dialogue sans image pour faire disparaître un personnage, ajoutez désormais un retrait explicite à cet endroit.

## Vérification reproductible

L'exemple `verify_dialogue_sprites` crée un projet de test dans un dossier neuf, avec quatre dialogues : image initiale, pin vide conservant l'image, double retrait suivi d'un dialogue hors champ, puis nouvelle image. Il utilise les deux images Mara indiquées dans ses chemins d'assets. Le test automatisé `dialogue_sprite_connection_preserves_sprite_until_explicit_removal` vérifie également qu'un autre personnage reste affiché.
