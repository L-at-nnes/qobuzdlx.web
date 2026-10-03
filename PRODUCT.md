# Product

## Register

product

## Users

People who pay for Qobuz and want the music on their own disks: self-hosters running the app in a container on a NAS or a home server, and listeners who build a FLAC library to play on their own hi-fi, in Plexamp, Jellyfin or a plain folder. Many of them listen to classical music, jazz and hi-res releases, where the metadata is the hard part: composers, works, movements, several discs, long titles, 24-bit masters.

They open the interface from a laptop at a desk or from a phone on the couch, on the home network or through a reverse proxy. The job is almost always the same: find a release, queue it, leave, and come back to a clean, correctly tagged folder. Sometimes it is a whole discography, a playlist of fifty tracks drawn from as many albums, or the favourites of an account.

## Product Purpose

QobuzDLX is a self-hosted web application that searches Qobuz and downloads what the account is entitled to into a tidy, tagged library. It exists so that the owner of a subscription can keep and organise the music without a desktop program, from any device in the house.

Success is a queue that runs unattended: every file is whole and checked, the quality that was really served is known, nothing is silently lost, and a failure says what failed and why. The interface has to make a long-running background job legible at a glance.

## Brand Personality

Calm, precise, quietly premium. A well-kept listening room, not a control room: warm light, good materials, nothing shouting. The voice is plain and exact, like the notes on the back of a good record sleeve. It states what is true (the bit depth and sampling rate that arrived, the file that failed) and never congratulates itself.

Three words: precise, unhurried, warm.

## Anti-references

- A generic admin template or "web-coded" dashboard: grey cards with drop shadows, identical tiles, a sidebar with five icons and nothing to say.
- The audio-app reflex: black background, neon green or orange accent, waveform decoration, glowing gradients.
- Streaming-service clones: a grid of big album art as the main surface, play buttons, a fake mini player. This is not a player.
- Skeuomorphic hi-fi: knobs, VU meters, brushed metal.
- SaaS marketing patterns inside a tool: hero metrics, gradient text, glassmorphism, emoji in the interface.
- Modals as the first answer to every question.

## Design Principles

1. **Queue it and leave.** The state of the work (running, waiting, done, failed, and where) reads from across the room. Progress is quiet and truthful, never decorative.
2. **Show what is true.** The quality that was served is not the quality that was asked for; a failed track names its reason. The interface reports facts in the user's words and hides raw API output.
3. **Dense and calm.** Lists of releases and tracks are the work. Rows carry the information (artist, year, tracks, bit depth, label) with room for long classical titles, instead of cards that repeat an image and two words.
4. **Familiar over clever.** Standard navigation, standard controls, standard keyboard behaviour. Delight is saved for a few moments, not spread across every page.
5. **Classical first.** Composer, work, movement, conductor and ensemble are the normal case. Titles wrap and stay readable; nothing is truncated into meaninglessness.
6. **Safe by default.** A password-protected, self-hosted tool that holds an account token is treated as sensitive: nothing secret is ever shown, and destructive actions say what they do.

## Accessibility & Inclusion

- WCAG 2.2 AA contrast in both themes; focus always visible; every action reachable by keyboard.
- Status is never carried by colour alone: icon and text accompany it.
- Honours `prefers-reduced-motion` and `prefers-color-scheme`; the theme can also be set by hand.
- French and English, complete, switchable at any time; layout survives longer French strings.
- Works on a phone: touch targets of at least 44 px, no horizontal scrolling, safe-area insets respected.
