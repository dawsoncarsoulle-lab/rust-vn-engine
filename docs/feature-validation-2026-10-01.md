# RVN extension candidate validation

The five feature families are present in the development engine and Blueprint
editor. This report records the validation performed on 1 October 2026. It is
not a final release approval: native Windows, standalone Chrome and Firefox,
and assistive technology checks remain outstanding.

## Candidate packages

The packages are working-tree builds, not tagged releases. Engine base commit:
`e0da44b24c4138ef2b202b5082a7769e20f1a2f0`. Editor base commit:
`78602240c7913d6cf11afa76c1902cca36e245f5`. Both have local changes; these commit
IDs alone do not reproduce the packages. The exact download files are identified
by their SHA256 values below. The installed application and public release were
not replaced during this validation.

| Package | SHA256 |
| --- | --- |
| rust-VN-development-linux-guard-x86_64.AppImage | e7079e248bb9ec80ddc054fc8502d8a54a1e35ae5a43e70f38e585c033288280 |
| rust-VN-development-windows-guard-x86_64.zip | 4b31696789098d0e7c10436e46ac123f855a524b6a42b8d1400983fae049b382 |

Each package includes the five bilingual, writable example projects, linked
source presentation, engine/CLI, Web runtime, and licensing notices. All five
examples pass strict checking before linking. The AppImage refuses to replace
an existing example destination. The Windows ZIP passes its archive integrity
check; that is not a Windows execution test.

## Automated validation

| Scope | Result | Evidence filename |
| --- | --- | --- |
| Engine workspace with native video enabled | 514 passed, 2 ignored, 0 failed | engine-source-guard-tests.log |
| Editor with native video enabled | 108 passed, 0 failed | editor-source-guard-tests.log |
| Distribution safety tests | 12 passed | desktop test_feature_examples and test_video_bundle |
| Dependency notice tests | 4 passed | tools test_sync_dependency_notices |
| Web video adapter simulations | Passed | tools/test-web-video.mjs |
| Browser semantics simulations | Passed | tools/test-web-semantics.mjs |
| Web speech adapter simulations | Passed | tools/test-web-speech.mjs |
| Linux Speech Dispatcher with an installed French voice | 1 explicitly enabled integration test passed | linux-speech-positive-test.log |

The two default ignored tests require an actual speech service or generate a
manual editor fixture. The speech test was separately enabled against the
available Linux service. It verifies voice selection, a real speech request,
client-scoped cancellation, Unicode handling and unavailable-language errors.
It does not certify intelligibility or compatibility with a screen reader.
JavaScript simulations do not substitute for browser or audio-device testing.

## Interactive validation

Local Linux tests ran on Pop!_OS 24.04 LTS x86_64 with the COSMIC/Wayland session.
All editor windows used fresh release builds and isolated projects; they were
closed through the application's remote protocol after testing.

| Scenario | Result and scope | Evidence |
| --- | --- | --- |
| Open a linked example from Home, return Home, Continue | Passed; virtual graph paths remain available in recents | recents-final-editor.log and captures |
| Launch directly on the end graph | Passed after correcting startup selection | roundtrip-editor.log |
| RVN → Blueprint → RVN → Blueprint | Passed; authored comment, node/pin IDs and existing placement retained | roundtrip-editor.log and isolated source presentation |
| Invalid external source | Passed in the exact AppImage; last valid graph retained, source/line diagnostic visible, save blocked | validation/appimage-invalid-source.png |
| Removed source file | Passed in the exact AppImage; cached graph retained, missing-file error visible, no source recreation | validation/appimage-missing-source.png |
| Repair and reload | Passed; stale source diagnostic removed | validation/appimage-valid-source.png and guard-appimage-editor.log |
| Native accessibility EN and FR | Passed scripted 26-step scenarios, including keyboard focus, large text, contrast and save compatibility | accessibility QA result files |
| Native layered characters and motion | Passed composition/motion scenarios and restoration checks | composition and motion QA result files |
| Native exported WebM player | Passed nine checks, including play, pause, seek, restoration and end state | video-export-final-captures/result.json |
| Interfaces in the in-app browser | Passed inventory, accented input, journal/modal ordering, puzzle state, save/reload and rollback observations | Live browser observations |
| WebM in the in-app browser | Passed real VP8 frames, pause at 0.8 seconds, save/reload to the same paused position, finish, stop and rollback | Live canvas and video element observations |

The independently exported native player has the same SHA256 as the packaged
player: `82a0e2d75379ecd8b9a041971f195849d81f19a53fc02caad78d7fc96e2e4850`.
The tested exported Web runtime and packaged Web runtime also match:
`6c98850c81f96257c3780f6f554d5c3beb011905939bffa8a97f04eb22b4afa9`.
Browser video element counts and timing were observed, but acoustic playback
and behavior in standalone Chrome/Firefox are not certified by those observations.

Detailed local evidence is retained in the candidate validation folder.
The auditable FFmpeg libraries and corresponding sources are packaged; the
decoder build is shared-library VP8/Vorbis without GPL or nonfree components.
Other codecs are not advertised as supported.

## Remaining delivery gates

- Run the exact Windows ZIP on Windows, including native speech, accented paths,
  source/Blueprint edits, all five example exports, save/rollback and video audio.
- Exercise HTTP exports in standalone Chrome and Firefox, including autoplay
  refusal, speech availability, keyboard/focus behavior and video restoration.
- Validate actual audio output and assistive technology behavior on the target
  systems; API acceptance alone is insufficient.
- Review source-linked authoring boundaries before advertising the entire plan
  as finished. Linking currently requires one self-contained source file. Visual
  scope renaming, interleaved declarations and edits to merged initialization
  blocks are explicitly refused rather than regenerated. Legacy workflows remain.
- Commit the reviewed source, rebuild reproducible packages, then repeat release
  acceptance on those exact files. Do not replace the public beta with this
  incomplete validation candidate.

## Résumé français

La version candidate contient les cinq familles et leurs exemples FR/EN.
Les tests moteur et éditeur passent, ainsi que les essais locaux décrits ci-dessus.
Le rechargement d'un script invalide ou absent conserve les graphes et affiche
désormais un diagnostic au lieu d'annoncer une réussite.

La livraison finale n'est pas validée : Windows réel, Chrome/Firefox indépendants,
l'audio perçu et les technologies d'assistance restent à contrôler. La liaison
RVN conserve encore des limites explicites. Les paquets ne sont pas publiés et
l'application installée n'a pas été remplacée.
