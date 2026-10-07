// Corpus Smogon figé pour les tests de légalisation (chantier A du PRD).
//
// Télécharge les équipes d'exemple de Smogon via l'API publique de crob.at
// (`/api/samples/{format}` puis `/api/team/{slug}`, comme app/src-tauri/src/teams.rs)
// et écrit au plus 50 sets par génération, répartis entre les paliers disponibles, dans
// crates/core/tests/data/smogon/gen{N}.txt (format d'export Showdown).
//
//   node scripts/smogon-corpus.mjs
//
// En octobre 2026, crob.at ne publie des équipes que pour ces paliers (les autres
// renvoient une liste vide) : Gen 4 OU, UU, LC ; Gen 5 à 7 OU, UU, RU.

import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const API = "https://crob.at/api";
const TIERS = ["ou", "uu", "ru", "nu", "ubers", "lc", "pu"];
const PER_GEN = 50;
const out = join(dirname(fileURLToPath(import.meta.url)), "..", "crates", "core", "tests", "data", "smogon");

async function json(url) {
  const r = await fetch(url, { headers: { "user-agent": "Kaleido (corpus de tests)" } });
  if (!r.ok) throw new Error(`${url} : HTTP ${r.status}`);
  return r.json();
}

/** Découpe un texte Showdown en sets (blocs séparés par une ligne vide, sans en-tête « === »). */
function sets(paste) {
  return paste
    .replace(/\r/g, "")
    .split(/\n\s*\n/)
    .map((b) => b.split("\n").filter((l) => !l.startsWith("===")).join("\n").trim())
    .filter((b) => b && !b.startsWith("==="));
}

mkdirSync(out, { recursive: true });
for (const gen of [4, 5, 6, 7]) {
  const byTier = [];
  for (const tier of TIERS) {
    const format = `gen${gen}${tier}`;
    let list = [];
    try {
      list = await json(`${API}/samples/${format}`);
    } catch (e) {
      console.warn(String(e));
    }
    if (!list.length) continue;
    const found = [];
    for (const s of list) {
      try {
        const team = await json(`${API}/team/${s.slug}`);
        for (const p of team.teams ?? []) for (const set of sets(p.paste)) found.push(set);
      } catch (e) {
        console.warn(String(e));
      }
    }
    byTier.push({ format, sets: [...new Set(found)] });
  }
  // Répartition équitable entre les paliers, dans l'ordre des équipes.
  const picked = [];
  for (let i = 0; picked.length < PER_GEN && byTier.some((t) => i < t.sets.length); i++) {
    for (const t of byTier) if (i < t.sets.length && picked.length < PER_GEN) picked.push({ format: t.format, set: t.sets[i] });
  }
  const text = byTier
    .map((t) => {
      const mine = picked.filter((p) => p.format === t.format).map((p) => p.set);
      return mine.length ? `=== [${t.format}] Smogon ===\n\n${mine.join("\n\n")}\n` : "";
    })
    .filter(Boolean)
    .join("\n");
  writeFileSync(join(out, `gen${gen}.txt`), text);
  console.log(`Gen ${gen} : ${picked.length} sets (${byTier.map((t) => `${t.format} ${picked.filter((p) => p.format === t.format).length}`).join(", ")})`);
}
