# Veyra identity quick guide

The symbol is a geometric `V` built from two controlled information channels. The
intentional break in the upper-right teal channel represents a policy boundary:
flow continues only through a verified gate. It is not a shield, lock, checkmark,
or lightning bolt.

## Production assets

- `veyra-symbol.svg` — primary two-color symbol on transparent background.
- `veyra-symbol-mono.svg` — one-color symbol using CSS `currentColor`.
- `veyra-icon.svg` — rounded-square application/editor/package icon.
- `veyra-lockup.svg` — horizontal symbol and lowercase wordmark.
- `veyra-docs-banner.svg` — dark, responsive documentation/repository header.
- `veyra-brand-board.png` — presentation board and visual reference.

## Palette

| Name | Hex | Use |
|---|---|---|
| Midnight | `#101525` | Primary wordmark, dark fields, UI grounding |
| Violet | `#9D8BFF` | Input/typed information channel |
| Teal | `#40D7D0` | Verified flow/output channel |
| Ice | `#F6F7FC` | Light presentation field |

Use solid fills only. Do not introduce gradients, glows, bevels, or shadows into
the production mark. On dark backgrounds, set the wordmark to Ice; the symbol
colors remain unchanged.

## Clear space and minimum size

Maintain clear space equal to the width of the teal gate segment on every side.
Use the symbol without the wordmark below 96 px. The icon is designed for 32 px
and larger; below 24 px use the monochrome symbol and simplify platform rounding.

The lockup uses Manrope/Inter-compatible geometry for live SVG text. Before
shipping immutable print assets, convert the approved wordmark to outlines in a
vector editor to prevent font substitution.

