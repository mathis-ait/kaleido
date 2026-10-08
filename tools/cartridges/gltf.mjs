import { readFileSync, writeFileSync } from "node:fs";
const [M = new URL("./sources", import.meta.url).pathname.slice(1), O = new URL("../../app/public/models", import.meta.url).pathname.slice(1)] = process.argv.slice(2);
const jobs = {
  ds: { src: "ds_game_card", images: ["base.jpg", "mr.jpg"] },
  "3ds": { src: "3ds_game_cartridge", images: ["base.jpg", "mr.jpg", "normal.jpg", "chip.jpg", "chip-mr.jpg", "chip-normal.jpg"] },
  gba: { src: "pokemon_cartridge_gameboy", images: ["base.jpg", "mr.jpg", "normal.jpg"] },
  switch: { src: "nintendo_switch_game_cartridge_v2", images: ["label.jpg", "label.jpg", "label.jpg", "label.jpg", "label.jpg"], keep: [3, 5, 8, 10] },
};
for (const [id, job] of Object.entries(jobs)) {
  const g = JSON.parse(readFileSync(`${M}/${job.src}/scene.gltf`, "utf8"));
  g.images.forEach((img, i) => { img.uri = job.images[i]; img.mimeType = "image/jpeg"; });
  if (job.keep) {
    // Une seule des cinq cartouches de la planche.
    const root = g.nodes.find((n) => n.name === "GLTF_SceneRootNode");
    root.children = job.keep;
  }
  writeFileSync(`${O}/${id}/scene.gltf`, JSON.stringify(g));
  console.log(id, g.images.map((x) => x.uri).join(","));
}
