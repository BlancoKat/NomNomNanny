# NomNom Nanny

A beautiful, private, cross-platform desktop app for tracking daily calories, macronutrients (protein, fat, carbs), dietary fiber, and hydration.

Built with **Tauri 2 + Svelte 5 + Tailwind + Rust + SQLite + USDA FoodData Central API**.

## Features

- Set personal daily goals for calories (kcal), protein/fat/carbs/fiber (g), and water (fl oz)
- Search real foods using the free USDA FoodData Central API (results are cached locally)
- Choose realistic servings and see live nutrient calculations
- Quick water logging + flexible manual/custom entry (including saved custom foods with per-100g scaling)
- Daily running totals with beautiful progress indicators
- 7-day and 30-day rolling averages
- View and edit history for past days
- 100% local data (SQLite) — works great offline after the first food lookups
- No accounts, no telemetry, no cloud sync required

## Getting Started

### Prerequisites

- [Node.js](https://nodejs.org/) 22 or newer
- [Rust](https://www.rust-lang.org/tools/install) (stable)
- On Linux: additional system dependencies for Tauri (see [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/))

### From Source

```bash
git clone https://github.com/blancokat/nomnomnanny.git
cd nomnomnanny
npm install
npm run tauri dev
```

The first run will take a few minutes while the Rust backend compiles.

> **Note**: Make sure `cargo` is in your `PATH`. On some systems you may need to run:
> ```bash
> source "$HOME/.cargo/env" && npm run tauri dev
> ```

### Production Build

```bash
npm run tauri build
```

The resulting installers will be in `src-tauri/target/release/bundle/`.

Pre-built binaries will be provided on GitHub Releases (https://github.com/blancokat/nomnomnanny/releases) once available.

### First Run

1. Launch the app.
2. Go to the **Goals** section and set your daily targets.
3. (Recommended) Get a free USDA API key at https://fdc.nal.usda.gov/api-key-signup and enter it in Settings. This enables the real food search.
4. Start logging! You can use the USDA search for real foods or Manual Entry for homemade/custom items.

## Custom Foods

You can save homemade recipes or frequently eaten items as "My Foods". These are stored with nutrients per 100g so you can log any quantity and have the app calculate the values for you. You can also override nutrients on a per-entry basis.

## Tech

- **Desktop runtime**: Tauri 2 (small native installers, ~10-20 MB)
- **Frontend**: Svelte 5 (runes) + Tailwind
- **Backend**: Rust + SQLite
- **Food data**: USDA FoodData Central (user-provided free API key)

## License

MIT

USDA data is public domain / U.S. government data.

---

Built with care. Eat well! 🥦💧
