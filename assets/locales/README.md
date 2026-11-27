# Translations

FluffyFrame's interface text lives in this folder, one JSON file per language. The language is picked in **Settings › Appearance › Language** and applies immediately.

| File | Language |
|---|---|
| `uk.json` | Українська |

English is the source language and has no file: the English text in the code is the key for every entry.

## File format

A locale file is a JSON object of **sections**, each mapping English text to its translation:

```json
{
  "Mastery": {
    "Mastered": "Освоєно",
    "Level {}/{}": "Рівень {}/{}"
  },
  "Game: planets and regions": {
    "Mars": "Марс"
  }
}
```

Sections only make entries easier to find. They are grouped by where the text shows (`Inventory`, `Settings`, `World: Bounties`…). **Common** holds text used in several places. The **Game:** sections hold names from the game data. The app ignores sections when it loads the file.

### Placeholders

`{}` marks a value the app fills in, in order. When your language needs them in another order, number them from 0:

```json
"Baro is at {} and leaves in {}.": "Через {1} Баро покине {0}."
```

Keep every placeholder, and keep leading and trailing spaces: some buttons are padded with them.

### Plurals

Text with a count is either a single string or a list of plural forms. The number and order of forms depend on the language's plural rule, which is set in `src/i18n.rs`:

| Rule | Languages | Forms |
|---|---|---|
| one / other | English and most others | `["1 item", "{} items"]` |
| one / few / many | Ukrainian (and Russian, Belarusian) | `["{} предмет", "{} предмети", "{} предметів"]` |

```json
"{} offers": ["{} пропозиція", "{} пропозиції", "{} пропозицій"]
```

### Missing entries

Anything a locale doesn't have shows in English, so a partial translation still works.

## Adding a language

1. Copy `uk.json` to `<code>.json` (e.g. `pl.json`). It already lists every string, grouped by section.
2. Translate the values. Keep the keys (the English text) as they are.
3. Add your language to `LOCALES` in `src/i18n.rs`: its code, its name in that language, the file, and a plural rule (add one if none fits).
4. Run `cargo test`. It fails if your file is missing a string from the app, has one the app no longer uses, or lists a string in two sections.

## Adding text to the app

Wrap display text in the code with:

- `tr("Text")` for plain text;
- `trf("Text with {}", &[&value])` for text with values;
- `trn("{} item", "{} items", n, &[&n])` for counted text (the entry is keyed by the plural form);
- `tkey("Text")` for text that is also data, translated where it's shown with `tr(value)`.

Then add the new strings to every locale file, in the section of the view that shows them, and run `cargo test`.

Cyrillic is covered by bundled fallback fonts (`assets/fonts/golos-*`, `exo2-*`). A script other than Latin or Cyrillic needs its own fallback font in `src/theme.rs`.

## What is not translated yet

Most game data comes from DE's English export and stays in English:

- **Inventory items**: item, mod, arcane, relic and resource names.
- **Missions**: node names (Apollodorus, Strata Relay), bounty jobs and challenges, Nightwave acts, sortie and alert details. Mission *types* (Survival, Interception…) and planets are translated.
- **Riven stats**: stat names on rivens, in the Riven Finder and in the Calculator (Critical Chance, Multishot…).

The community export the app already reads (warframe-public-export-plus) ships DE's own translations as `dict.<code>.json`, so these could follow the game's official wording later.

Game terms are a choice for each language. The Ukrainian file keeps trading terms (WTS, WTB) and some proper names in English, and uses the game's official names where they exist.
