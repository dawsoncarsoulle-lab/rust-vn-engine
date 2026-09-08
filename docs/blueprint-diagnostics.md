# Diagnostics Blueprint — 8 septembre 2026

## Utilisation

Le menu **v**, à droite de **Compiler**, propose **Compiler le graphe courant** et **Tout compiler (projet)**. Le second mode est sélectionné par défaut. La sélection lance le contrôle ; le bouton Compiler réutilise ensuite ce choix. Les flèches du clavier, Entrée et Échap fonctionnent aussi dans ce menu. Une indication distingue le graphe courant valide du projet non encore vérifié.

**Jouer** et **Exporter projet** compilent les graphes du projet, y compris les documents ouverts non enregistrés. Une erreur localisée dans un graphe fermé conserve son chemin et son identifiant de nœud. Les graphes fautifs et leurs dossiers parents sont signalés en rouge avec **!** ; un onglet fautif ouvert est également rouge.

Dans **Diagnostics**, la liste défilante à gauche présente les résultats et la zone de droite présente le détail sélectionné. Cliquez sur une ligne pour ouvrir le graphe, sélectionner le nœud et le cadrer avec un zoom lisible. Une bande rouge **ERROR!** apparaît sous les nœuds concernés. L’ouverture d’un graphe fautif cadre son premier nœud en erreur. Les erreurs générales du projet ne sont pas attribuées arbitrairement à un nœud.

La molette et les barres déplaçables permettent de parcourir la liste, ses détails, le script généré, les résultats de recherche et le journal CLI. Les lignes du script et du journal sont repliées pour conserver les textes longs.

La préparation des anciens graphes est identique à l’ouverture et à la compilation : en particulier, les anciennes déclarations explicites de sprites sont migrées en connexions visibles. La compilation travaille sur une copie en mémoire, sans enregistrer ces migrations. Cette harmonisation supprime les faux diagnostics qui disparaissaient à l’ouverture. Une connexion réellement retirée n’est pas recréée : son erreur reste signalée jusqu’à correction.

Chaque résultat présente sa gravité, son code, le problème, le chemin du graphe, la cible et une aide. La molette fait défiler les détails longs. Les accents et chemins longs sont conservés. Les repères du graphe actif sont recalculés après modification. Recompilez le projet après une modification externe aux graphes ouverts.

Exemple : `missing_input_value`, graphe `ch1_aria_intro`, nœud 2, entrée Sprite de « Afficher un sprite ». L’aide propose de relier une image depuis Assets. Cela ne signifie pas qu’un simple dialogue doit afficher un sprite : le nœud d’affichage explicite, lui, doit disposer d’une image.

Aucune suggestion n’est appliquée automatiquement. Les fichiers de l’histoire ne sont pas corrigés ou réécrits par la navigation dans les diagnostics. Les mécanismes habituels de migration en mémoire et de récupération des graphes restent actifs.

## Recherche : conventions retenues

- **Unreal — Compiler Results** : liste des erreurs et avertissements, informations complémentaires, liens vers le graphe et le nœud, ouverture du panneau après échec. C’est la référence pour la localisation et la navigation. [Documentation Epic](https://dev.epicgames.com/documentation/unreal-engine/compiler-results?application_version=4.27).
- **Rust — structure du diagnostic** : distinguer message principal, emplacement, code, notes et aide. Une suggestion incertaine ne doit pas devenir une correction automatique. Ici, les codes existants sont conservés et des aides françaises sont associées aux familles d’erreurs courantes. [Guide des diagnostics](https://rustc-dev-guide.rust-lang.org/diagnostics.html), [structures et sous-diagnostics](https://rustc-dev-guide.rust-lang.org/diagnostics/diagnostic-structs.html).
- **Unreal — débogage pendant le jeu** : points d’arrêt, pas à pas, valeurs surveillées, pile d’appels et trace d’exécution sont distincts des diagnostics de compilation. Ils nécessitent une connexion moteur–éditeur et ne sont pas fournis par cette modification. [Blueprint Debugging Example](https://dev.epicgames.com/documentation/en-us/unreal-engine/blueprint-debugging-example-in-unreal-engine).
- Les erreurs d’exécution sont elles aussi une catégorie distincte : Unreal expose notamment les accès invalides, boucles infinies et interruptions. Ne pas les présenter comme de simples erreurs de compilation. [Types d’exceptions Blueprint](https://dev.epicgames.com/documentation/unreal-engine/API/Runtime/CoreUObject/EBlueprintExceptionType__Type).

## Périmètre et limites

- La compilation poursuit le contrôle des autres graphes après un échec de transpilation. Le transpileur peut encore ne retourner que la première erreur sémantique d’un même graphe ; les diagnostics structurels retournés ensemble sont conservés.
- Les erreurs de lecture de fichiers, d’import et de validation globale restent des diagnostics de projet lorsqu’aucune cible fiable n’est disponible.
- Les familles courantes disposent de messages français et d’aides ciblées ; les cas non encore traduits conservent un détail technique avec une aide générale. Ce n’est pas encore la richesse complète de `rustc` pour chaque cas.
- Pas encore de liste filtrable par gravité, de correctifs automatiques avec niveau de confiance, ni d’index exhaustif d’explications par code.
- Les erreurs de graphes non ouverts sont actualisées lors de la compilation du projet, pas par une surveillance permanente du disque.

## Vérifications

33 tests de l’éditeur passent, dont un test supplémentaire comparant la préparation d’un ancien graphe fermé à sa migration d’ouverture, son idempotence et la persistance d’une vraie déconnexion. Le test de non-régression de migration des sprites dans rvn_graph passe également.

Audit natif du 8 septembre sur des copies indépendantes de La Dernière Archiviste :

- Compilation du graphe courant puis du projet entier, sélection à la souris et au clavier.
- Projet original copié : suppression des 15 faux diagnostics de migration, compilation réussie et lancement/arrêt du véritable moteur.
- Copie avec une déconnexion volontaire et dix graphes de test supplémentaires : 11 erreurs réelles, graphes fermés signalés, navigation entre onglets, liste jusqu’à la dernière erreur, nœud cadré et bandeau visible.
- Suppression du nœud fautif : 10 erreurs ; annulation : 11 erreurs et retour du bandeau.
- Script généré : défilement jusqu’à la dernière ligne, à la molette et avec la barre.
- Recherche : défilement et ouverture du bon résultat après défilement.
- Journal du moteur réel consulté ; journal synthétique de 200 lignes utilisé séparément pour contrôler le défilement jusqu’à la ligne 200.
- Vérification visuelle à 1280×650 et 1920×976. Les fenêtres de test ont été fermées.

Traces de cet audit : `/tmp/rvn-debug-audit.Lc6XXs/`. Les captures natives sont dans `/tmp/makepad-remote/`. Aucun graphe de l’histoire originale n’a été modifié pour ces essais.
