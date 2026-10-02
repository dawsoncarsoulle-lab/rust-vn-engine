# Accessibility example

This bilingual example exercises the same accessibility commands available in
RVN scripts and Blueprints. It contains no external media.

## Player controls

Press **F8** to open the accessibility panel, including from a custom title menu.
Use **Tab**, **Shift Tab** or the up and down arrow keys to navigate. Use the left
and right arrow keys to adjust a setting, or **Enter** to activate it. **Escape**
or **F8** closes the panel without advancing the story. The mouse and mouse wheel
also work. On a small window, the panel reveals the focused setting by scrolling.

Text size ranges from 75 to 250 percent. High contrast and reduced motion are
optional. Speech rate and volume are independent of prerecorded dialogue audio.
**V** toggles self-voicing, unless an editable text field has focus. The panel can
read the current dialogue again or stop speech.
Long dialogues stay within the window. Use **Page Down / Page Up** to scroll
when their enlarged text is taller than the available space.

Preferences are saved separately from story progress. Loading a save or using
rollback restores authored defaults but retains the player's override. Reset
restores the project's settings. Speech requests are transient and are not
replayed from a saved history.

Speech uses Speech Dispatcher on Linux, Windows SAPI on Windows, and Web Speech
in browsers. An unavailable service or language produces a visible diagnostic;
the game does not silently pretend that it spoke. Voices differ by platform.
Browser voice availability and autoplay permissions depend on the browser.

## Authoring

The example uses `accessibility.configure(properties)` and localized accessible
labels on interface components. `accessibility.speak(text)` and
`accessibility.stop()` are also available in narrative code and non-blocking
handlers. Calculation expressions cannot perform these operations.

Blueprints expose **Configure accessibility**, **Speak text** and **Stop speech**.
The configure inspector edits the same validated dictionary as RVN. Computed
expressions remain connected rather than being overwritten by inspector edits.

This is development content, not a certified accessibility conformance claim.
Real Windows, screen-reader and browser acceptance tests must accompany the
exact packages before release.

## Exemple en français

**F8** ouvre le panneau, même depuis un menu de titre personnalisé. **Tab**, **Maj
Tab** et les flèches verticales déplacent le focus. Les flèches horizontales
règlent la valeur ; **Entrée** active le contrôle. **Échap** ou **F8** ferme le
panneau sans avancer dans l’histoire. La souris et sa molette sont disponibles.
Le panneau défile pour rendre le contrôle ciblé visible dans une petite fenêtre.

La taille du texte va de 75 à 250 %. Le contraste élevé et la réduction des
mouvements sont optionnels. **V** active la lecture vocale, sauf pendant la saisie
d’un texte. Le panneau permet de relire le dialogue et d’arrêter la voix.
Si un dialogue agrandi dépasse la fenêtre, **Page suivante / Page précédente**
permet de le faire défiler sans avancer dans l’histoire.

Les préférences du joueur sont indépendantes des sauvegardes narratives. Le
chargement et le retour arrière ne les effacent pas ; la réinitialisation
rétablit les réglages du projet. Une ancienne lecture vocale n’est jamais rejouée
en restaurant une sauvegarde.

Le moteur utilise Speech Dispatcher sur Linux, SAPI sur Windows et Web Speech
dans le navigateur. L’absence de service ou de voix dans la langue demandée
produit un message visible. Les voix et les permissions de lecture varient selon
la plateforme. Une compilation Windows ne remplace pas un essai Windows réel.

En RVN comme en Blueprints, les commandes configurent l’accessibilité, lisent un
texte localisable et arrêtent la voix. Les composants exposent des libellés
accessibles localisables. Le moteur reste MIT ; l’éditeur conserve sa licence
propriétaire. Cet exemple n’est pas une certification de conformité.
