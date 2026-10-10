import { computed } from "vue";
import { allGames, titleIdOf } from "../games";
import { isKaleidoRom, type Detection } from "../types";

/** Jeux Switch reconnus à leur Title ID de base. */
const SWITCH_GAMES: Record<string, string> = {
  "010003F003A34000": "letsgopikachu",
  "0100187003A36000": "letsgoeevee",
  "0100ABF008968000": "sword",
  "01008DB008C2C000": "shield",
  "0100000011D90000": "brilliantdiamond",
  "010018E011D92000": "shiningpearl",
  "01001F5010DFA000": "legendsarceus",
  "0100A3D008C5C000": "scarlet",
  "01008F6008C5E000": "violet",
};

/** Identifiant de jeu de la Living Dex d'une ROM ou d'un jeu de la bibliothèque. */
export function livedexGameOf(d: Detection): string | null {
  if (d.game?.id) return d.game.id.replace(/_/g, "");
  const tid = titleIdOf(d);
  return tid ? (SWITCH_GAMES[tid] ?? null) : null;
}

/**
 * Jeux possédés (bibliothèque), par identifiant : la première ROM d'origine trouvée. Les ROMs
 * randomisées sont écartées : leurs rencontres ne sont plus celles du jeu.
 */
export const ownedGames = computed(() => {
  const map = new Map<string, Detection>();
  for (const d of allGames.value) {
    if (isKaleidoRom(d) || d.romhack) continue;
    const id = livedexGameOf(d);
    if (id && !map.has(id)) map.set(id, d);
  }
  return map;
});
