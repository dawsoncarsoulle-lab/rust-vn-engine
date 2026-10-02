# L’Atlas des Brumes — ressources

Cette aventure et son univers sont originaux. Le menu s’inspire de l’organisation
en pages d’un atlas d’aventure ; aucune image, musique, interface, marque ou
ressource Nintendo ou Epic n’est incluse.

## Illustrations

Les illustrations ont été produites par génération assistée spécialement pour
cette aventure, à partir de descriptions originales. Les retouches des tenues et
des expressions conservent la position du personnage et le canevas des calques.
Les portraits et les icônes ont un véritable fond transparent.

Dans le dépôt source du moteur, `ASSET-PROMPTS.json` conserve les prompts
disponibles, les briefs résumés lorsque le texte exact n’a pas été conservé,
les références des retouches, les identifiants de génération et les copies
utilisées dans le jeu. Ce manifeste n’est pas nécessaire au paquet jouable
ou à l’exemple installé dans l’éditeur.

Inventaire livré :

- `scenes/harbor.png`, `scenes/forest.png`, `scenes/temple.png` : port, forêt et sanctuaire ;
- `maps/world.png` : carte insulaire sans noms empruntés ;
- `ui/frame.png` : cadre ornemental de l’Atlas ;
- `portraits/keeper.png`, `portraits/player.png` : Maëlys et le cartographe ;
- `layers/keeper_body.png`, `keeper_coat.png`, `keeper_neutral.png`, `keeper_smile.png`, `keeper_lantern.png` : corps, tenue, visages et accessoire alignés ;
- `icons/boussole.png`, `lanterne.png`, `grappin.png`, `sceau.png`, `cle.png`, `cristal.png` : objets originaux de la quête ;
- `sprites/lantern/default.png`, `bright.png` : deux intensités de la même lanterne pour son animation ;
- `cgs/atlas_memory.png` : illustration originale du souvenir débloqué.

Les autres petits pictogrammes du menu sont dessinés par des formes vectorielles
RVN originales, éditables dans le projet ; ce ne sont pas des images remplacées
par des captures d’un autre jeu. Aucune ressource graphique tierce n’est requise.

## Musique, effet et vidéo

`music/fog_theme.wav` et `sfx/beacon.wav` sont des synthèses musicales originales,
sans échantillon externe : une petite phrase d’arpèges et une réponse de balise.
Le dépôt source du moteur conserve les fréquences, les enveloppes et le niveau
audio utilisés dans `assets/generate_media.py` ; ce script de construction
n’est pas nécessaire au paquet jouable ou à l’exemple installé. La vidéo
`movies/prologue.webm` est un travelling léger de la
scène originale du port, avec fondu, audio original et sous-titres français et
anglais. Format : VP8, 1280 × 720, 24 images/s, audio Vorbis stéréo 24 kHz,
durée 6,011 s. Elle ne contient aucun extrait de film ni enregistrement emprunté.

Les fichiers de sous-titres `.srt` et `.vtt` sont également conservés. Les
sous-titres affichés dans le jeu sont définis en RVN et traduits par ses locales.

## Polices

Aucune police supplémentaire n’est distribuée par cet exemple. Les textes
emploient les polices intégrées au moteur, dont les licences accompagnent le
paquet de l’application. Le rendu et la disponibilité de la lecture vocale
dépendent du système ; aucune voix ni transcription audio tierce n’est fournie.

## Réutilisation

Le code RVN, les thèmes, les traductions, les illustrations originales et les
médias synthétisés de cet exemple sont fournis sous la licence MIT du dépôt.
Les licences des dépendances et des polices intégrées restent distinctes.
