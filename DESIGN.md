---
name: QobuzDLX
description: A calm, warm interface for queueing hi-res music and leaving it to run.
colors:
  night-room: "oklch(0.19 0.012 65)"
  night-surface: "oklch(0.225 0.013 65)"
  night-raised: "oklch(0.26 0.014 65)"
  night-line: "oklch(0.34 0.014 65)"
  night-text: "oklch(0.93 0.010 75)"
  night-muted: "oklch(0.74 0.014 70)"
  night-faint: "oklch(0.63 0.014 70)"
  paper: "oklch(0.975 0.008 80)"
  paper-surface: "oklch(0.992 0.005 80)"
  paper-sunken: "oklch(0.952 0.010 78)"
  paper-line: "oklch(0.88 0.012 75)"
  paper-text: "oklch(0.24 0.015 60)"
  paper-muted: "oklch(0.46 0.016 65)"
  paper-faint: "oklch(0.55 0.015 65)"
  varnish: "oklch(0.74 0.115 62)"
  varnish-deep: "oklch(0.52 0.125 52)"
  success: "oklch(0.70 0.12 155)"
  warning: "oklch(0.78 0.13 85)"
  danger: "oklch(0.68 0.16 28)"
typography:
  title:
    fontFamily: "Newsreader Variable, Georgia, serif"
    fontSize: "1.375rem"
    fontWeight: 500
    lineHeight: 1.25
  item:
    fontFamily: "Newsreader Variable, Georgia, serif"
    fontSize: "1.0625rem"
    fontWeight: 500
    lineHeight: 1.3
  body:
    fontFamily: "Hanken Grotesk Variable, system-ui, sans-serif"
    fontSize: "0.9375rem"
    fontWeight: 400
    lineHeight: 1.5
  label:
    fontFamily: "Hanken Grotesk Variable, system-ui, sans-serif"
    fontSize: "0.8125rem"
    fontWeight: 500
    lineHeight: 1.35
rounded:
  sm: "6px"
  md: "10px"
  lg: "14px"
spacing:
  xs: "4px"
  sm: "8px"
  md: "16px"
  lg: "24px"
  xl: "40px"
components:
  button-primary:
    backgroundColor: "{colors.varnish}"
    textColor: "{colors.night-room}"
    rounded: "{rounded.md}"
    padding: "9px 16px"
  button-quiet:
    backgroundColor: "{colors.night-raised}"
    textColor: "{colors.night-text}"
    rounded: "{rounded.md}"
    padding: "9px 16px"
  field:
    backgroundColor: "{colors.night-room}"
    textColor: "{colors.night-text}"
    rounded: "{rounded.md}"
    padding: "10px 12px"
---

# Design System: QobuzDLX

## 1. Overview

**Creative North Star: "The Listening Room"**

The interface is a well-kept room where you arrange the evening's music and then stop looking at it. Surfaces are warm, low and matte, like wood and paper under a lamp; one accent, the colour of violin varnish, marks what can be acted on and what is current. Nothing glows, nothing bounces. The room is meant to be left, so what remains visible while you are away is the state of the work: what is running, what waits, what failed.

Density is deliberate. Albums and tracks are the content, so they are set as rows with room for long titles in a text serif, the way a record sleeve sets works and movements, and the controls around them stay in a plain, warm sans. Cards are used only where a thing is genuinely a unit apart (the album being downloaded). Everything else is a list on a surface.

The system rejects the generic admin template (grey tiles and shadows), the audio-app reflex (black with neon), streaming-service clones (artwork grids and play buttons) and skeuomorphic hi-fi (knobs and meters).

**Key Characteristics:**
- Two themes from one set of tokens: "night room" (dark, warm brown-grey) and "paper" (light, warm off-white). Never pure black or white.
- One accent, used for the primary action, the current selection and live progress, and nowhere else.
- Text serif for the titles of music, sans for everything that is an interface.
- Flat by default; depth comes from surface tone and a hairline, not from shadows.
- Motion only states a change; it is short and eased out.

## 2. Colors

A restrained palette: warm tinted neutrals carry the whole surface, and a single amber-brown accent speaks.

### Primary
- **Violin Varnish** (oklch(0.74 0.115 62) on the night room, oklch(0.52 0.125 52) on paper): the primary button, the current item, the progress bar, links in prose. Its rarity is the point.

### Neutral
- **Night Room** (oklch(0.19 0.012 65)): the page in the dark theme.
- **Night Surface** (oklch(0.225 0.013 65)) and **Night Raised** (oklch(0.26 0.014 65)): the second and third layers: panels, then controls and open rows.
- **Night Line** (oklch(0.34 0.014 65)): hairlines and borders.
- **Night Text / Muted / Faint** (oklch(0.93 0.010 75) / oklch(0.74 0.014 70) / oklch(0.63 0.014 70)): primary text, secondary text, tertiary text. Faint is for metadata that is never essential.
- **Paper** (oklch(0.975 0.008 80)), **Paper Surface** (oklch(0.992 0.005 80)), **Paper Sunken** (oklch(0.952 0.010 78)) and **Paper Line** (oklch(0.88 0.012 75)): the light theme.
- **Paper Text / Muted / Faint** (oklch(0.24 0.015 60) / oklch(0.46 0.016 65) / oklch(0.55 0.015 65)): the light theme's text.

### Semantic
- **Success** (oklch(0.70 0.12 155)), **Warning** (oklch(0.78 0.13 85)), **Danger** (oklch(0.68 0.16 28)): states of a download. Each is always paired with an icon and a word; each has a deeper variant on paper to keep AA contrast.

### Named Rules
**The One Voice Rule.** The accent covers at most a tenth of any screen. If two things are the accent, neither is.

**The Tinted Neutral Rule.** No neutral is grey. Every surface and text colour leans warm by a few points of chroma, and no colour is `#000` or `#fff`.

**The Status Is Not A Colour Rule.** A state is told by an icon and a word first; the colour confirms it.

## 3. Typography

**Display / Content Font:** Newsreader Variable (with Georgia, serif)
**Body / Interface Font:** Hanken Grotesk Variable (with system-ui, sans-serif)

**Character:** a bookish text serif for the names of things in the music, and an even, slightly warm grotesque for the machine around them. The serif is content, never a label.

### Hierarchy
- **Title** (Newsreader 500, 1.375rem, 1.25): page headings and the name of the album in progress.
- **Item** (Newsreader 500, 1.0625rem, 1.3): the title of an album, track, playlist or queue entry. Wraps; never truncated to one line.
- **Body** (Hanken Grotesk 400, 0.9375rem, 1.5): sentences and descriptions, at most 70ch.
- **Label** (Hanken Grotesk 500, 0.8125rem, 1.35): buttons, field labels, metadata under an item, tabs. Numbers use tabular figures.

### Named Rules
**The Serif Is Content Rule.** Newsreader sets the names of music only. Buttons, labels, fields and navigation are never serif.

**The Long Title Rule.** A classical title ("Symphony No. 7 in A Major, Op. 92: II. Allegretto") wraps onto a second line and keeps its meaning. Ellipsis is for metadata, not for titles.

## 4. Elevation

Flat by default. Depth is tonal: page, surface, raised, in that order, separated by a hairline. A shadow is reserved for the one thing that floats above the page: a menu or the toast that confirms an action. Nothing at rest casts a shadow.

### Shadow Vocabulary
- **Float** (`box-shadow: 0 8px 24px oklch(0.1 0.01 65 / 0.35)`): menus and toasts only.

### Named Rules
**The Flat-At-Rest Rule.** If a resting surface has a shadow, remove it and use the next tone and a hairline.

## 5. Components

### Buttons
- **Shape:** gently rounded (10px), 40px high on desktop, 44px on touch.
- **Primary:** Violin Varnish fill, Night Room text. One per view region.
- **Quiet:** Night Raised fill with a hairline, the default for anything secondary.
- **Ghost:** no fill, text only, for row actions.
- **Hover / Focus:** tone shifts by one step in 150ms; focus is a 2px accent ring offset by 2px, always visible.
- **Disabled / Loading:** muted fill and text, no pointer; a loading button keeps its width and shows a small determinate-free spinner next to the label.

### Fields
- **Style:** Night Room fill (sunken), hairline border, 10px radius, 40px high.
- **Focus:** border becomes the accent and the 2px ring appears.
- **Error:** danger border plus a message below with an icon; never colour alone.

### Rows (releases and tracks)
- **Structure:** thumbnail 48px, title in the item style, a line of metadata in the label style (artist, year, tracks, bit depth / sampling rate), actions at the end.
- **Selected:** a tone step up and a checked box, no side stripe.
- **Expanded:** the track list opens inline under the row, separated by a hairline.
- **Done mark:** a check icon and the word "downloaded", sibling of the button.

### Progress
- **Bar:** a 6px track in Night Raised with an accent fill; the transfer speed and percentage in tabular figures beside it. Never animated beyond the width change.

### Navigation
- **Desktop:** a quiet top bar with the name, three destinations (Search, Downloads, Settings) with a count badge on Downloads, and the theme and language controls at the end.
- **Phone:** the same three destinations as a bottom bar with labels, safe-area aware.

### Toasts
- One at a time, bottom centre, Float shadow, announced to assistive technology.

## 6. Do's and Don'ts

### Do:
- **Do** set titles of music in Newsreader and let them wrap.
- **Do** tell state with an icon and a word; colour confirms.
- **Do** keep the accent to the primary action, the current selection and live progress.
- **Do** show the quality that was really served and the reason a track failed, in plain words.
- **Do** keep every control reachable by keyboard with a visible focus ring.
- **Do** use tonal layers and a hairline for depth.

### Don't:
- **Don't** make a generic admin template or "web-coded" dashboard: grey cards with drop shadows, identical tiles.
- **Don't** use the audio-app reflex: black background, neon green or orange accent, waveforms, glowing gradients.
- **Don't** build a streaming-service clone: artwork grids as the main surface, play buttons, a fake mini player.
- **Don't** use skeuomorphic hi-fi: knobs, VU meters, brushed metal.
- **Don't** put SaaS marketing patterns in a tool: hero metrics, gradient text, glassmorphism, emoji.
- **Don't** reach for a modal first; use inline expansion, a sheet that stays on the page, or a confirmation next to the action.
- **Don't** use `border-left` or `border-right` wider than 1px as a coloured stripe on rows or callouts.
- **Don't** use `#000`, `#fff` or an untinted grey.
- **Don't** use an em dash in interface copy.
