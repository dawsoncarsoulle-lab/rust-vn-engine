# Video playback example

This development example exercises WebM VP8/Vorbis playback in the RVN script
and corresponding Blueprint nodes. It needs the extended engine and editor,
not the published 0.2.5 editor. The short procedural clip is a test pattern
with synthetic audio, not a third-party film.

## Run on desktop

Use the matching development package, keeping its media libraries and notices
beside the executables. From that package's folder:

```sh
./rvn check --strict /path/to/videos
./rvn_bevy /path/to/videos
./rvn build --target desktop /path/to/videos
```

On Windows use `rvn.exe` and `rvn_bevy.exe`. Extract the entire application
folder; do not copy only the executable. An exported game includes the shared
decoder, its corresponding sources, build instructions and separate notices.
Keep those files when redistributing the game. Media you add needs its own
redistribution rights.

For a source build, build the audited shared decoder using
`tools/build-ffmpeg-lgpl.sh`, set `FFMPEG_DIR` to its absolute prefix, and build
`rvn_cli` and `rvn_bevy` in release mode with `--features video`.

## Check playback and restored state

1. Start a new game and check both picture and sound. The video uses the same
   playback clock for its audio and subtitles.
2. Advance once to pause. Both picture and audio should stop.
3. Advance again: the clip seeks to 0.8 seconds while paused, with volume at 30%.
4. Save here, close the game, reopen and load. Check the paused position and
   volume, and verify that loading does not create a second audio stream.
5. Continue. Playback resumes, waits for completion, and keeps the last frame.
   Its end handler increments `completed` once. The next dialogue stops and
   removes the player. Rollback restores the earlier playback state.
6. The final cinematic is blocking and skippable. Its dialogue appears only
   after playback finishes or is skipped.

## Web export

```sh
./rvn build --target web /path/to/videos
```

Serve the printed export directory over HTTP. Opening `index.html` directly is
not supported. Browsers may require a user gesture before playing audio/video;
use the visible resume button if automatic playback is refused. The Web player
uses browser media APIs and does not ship the desktop FFmpeg libraries.

## Blueprint authoring

Link `main.rvn` explicitly from the development editor's Scripts panel after
saving existing graphs. The clip properties, playback commands and handler
are available in their corresponding graph nodes. A literal **Video clip**
node exposes resource selection, subtitles, volume, layering and playback
flags. Save explicit visual edits; compilation alone does not rewrite the
linked source. Resolve any external-source conflict before saving.

## Exemple vidéo en français

Cet exemple de développement utilise un motif vidéo procédural et un son
synthétique en WebM VP8/Vorbis. Il nécessite le moteur et l'éditeur étendus,
pas l'éditeur public 0.2.5.

Conservez les bibliothèques vidéo et les notices du paquet. Lancez une partie,
puis avancez pour mettre la vidéo en pause et déplacer sa lecture à 0,8 seconde,
avec un volume de 30 %. Sauvegardez, fermez, relancez et chargez : la position,
la pause et le volume doivent revenir sans doubler le son. Continuez pour
vérifier l'attente de fin, le dernier photogramme, l'événement de fin unique et
la suppression du lecteur. Le retour arrière restaure aussi l'état vidéo.
La cinématique finale bloque le récit jusqu'à sa fin ou son saut autorisé.

L'export Web se sert en HTTP ; le navigateur peut demander un clic avant la
lecture. Utilisez alors son bouton de reprise. Dans l'éditeur de développement,
liez explicitement `main.rvn` depuis Scripts et utilisez les nœuds vidéo et
leurs inspecteurs. L'export desktop conserve les sources et notices du décodeur
dans le jeu distribué. Vérifiez séparément les droits des médias que vous ajoutez.
