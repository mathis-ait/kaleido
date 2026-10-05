<script setup lang="ts">
import { computed } from "vue";
import Icon from "../../components/Icon.vue";
import Sprite from "../../components/Sprite.vue";
import Tip from "../../components/Tip.vue";
import { formatDate, isShiny, type GiftDetails } from "./api";
import { GIFT_TERMS } from "./terms";

/** Panneau de droite : fiche complète d'une carte et actions. */
const props = defineProps<{ gift: GiftDetails; busy: boolean; hasSave: boolean; lastAdded: boolean }>();
const emit = defineEmits<{ add: []; export: []; edit: [] }>();

const p = computed(() => props.gift.pokemon);
const shiny = computed(() => isShiny(props.gift.shiny));
const date = computed(() => formatDate(props.gift.date));
const genderSymbol = (g: string | null) => (g === "male" ? "♂" : g === "female" ? "♀" : "");
const addDisabled = computed(() => props.busy || !props.hasSave || props.gift.compatible === false);
const addHint = computed(() => {
  if (!props.hasSave) return "Ouvre une sauvegarde pour recevoir ce cadeau.";
  if (props.gift.compatible === false) return props.gift.incompatibleReason ?? "";
  return props.gift.kind === "item" ? "Les objets sont ajoutés au sac." : "Rangé dans la première case libre des boîtes.";
});
</script>

<template>
  <aside class="details sv-panel">
    <header class="head">
      <div class="art" :class="`f-${gift.formatLabel.toLowerCase()}`">
        <Sprite v-if="gift.species" :id="gift.species" :shiny="shiny" :size="112" />
        <Icon v-else :name="gift.kind === 'item' ? 'bag' : 'gift'" :size="56" />
        <span v-if="gift.egg" class="egg-badge"><Icon name="egg" :size="16" /></span>
      </div>
      <div class="meta">
        <div class="badges">
          <span class="fmt" :class="`f-${gift.formatLabel.toLowerCase()}`">{{ gift.formatLabel }}</span>
          <span v-if="gift.cardId" class="card-no">Carte n°{{ gift.cardId }}</span>
          <span class="kind">{{ gift.kindLabel }}</span>
        </div>
        <h3>{{ gift.title }}</h3>
        <small v-if="p">
          {{ gift.speciesName }}<template v-if="gift.formName"> ({{ gift.formName }})</template>
          · {{ p.level ? `N. ${p.level}` : "niveau au hasard" }}
          <template v-if="shiny"> · <Icon name="star" :size="12" /> chromatique</template>
        </small>
      </div>
    </header>

    <div class="scroll">
      <section>
        <h4>Jeux <Tip :title="GIFT_TERMS.distribution.title" :text="GIFT_TERMS.distribution.text" /></h4>
        <div class="chips">
          <span v-for="g in gift.games" :key="g.id" class="game">{{ g.name }}</span>
        </div>
      </section>

      <section v-if="gift.itemNames.length">
        <h4>Objets</h4>
        <ul class="list">
          <li v-for="(n, i) in gift.itemNames" :key="i">{{ n }}</li>
        </ul>
      </section>

      <template v-if="p">
        <section>
          <h4>Pokémon</h4>
          <dl>
            <dt>Espèce</dt>
            <dd>{{ gift.speciesName }}<template v-if="gift.formName"> ({{ gift.formName }})</template></dd>
            <dt>Niveau</dt>
            <dd>{{ p.level || "Au hasard" }}<template v-if="p.metLevel && p.metLevel !== p.level"> (rencontré N. {{ p.metLevel }})</template></dd>
            <template v-if="p.nickname">
              <dt>Surnom</dt>
              <dd>{{ p.nickname }}</dd>
            </template>
            <dt>Œuf</dt>
            <dd>{{ p.egg ? "Oui" : "Non" }}</dd>
            <dt>Chromatique <Tip :title="GIFT_TERMS.shinyLock.title" :text="GIFT_TERMS.shinyLock.text" /></dt>
            <dd :class="{ gold: shiny }">{{ gift.shinyLabel }}</dd>
            <dt>Sexe</dt>
            <dd>{{ gift.genderLabel }}</dd>
            <dt>Nature</dt>
            <dd>{{ gift.natureName }}</dd>
            <dt>Talent</dt>
            <dd>{{ gift.abilityLabel }}</dd>
            <dt>IV <Tip :title="GIFT_TERMS.perfectIvs.title" :text="GIFT_TERMS.perfectIvs.text" /></dt>
            <dd>{{ gift.ivsLabel }}</dd>
            <dt>Objet tenu</dt>
            <dd>{{ gift.heldItemName ?? "Aucun" }}</dd>
            <dt>Ball</dt>
            <dd>{{ gift.ballName ?? "—" }}</dd>
          </dl>
        </section>

        <section>
          <h4>Dresseur d'origine <Tip :title="GIFT_TERMS.ot.title" :text="GIFT_TERMS.ot.text" /></h4>
          <dl>
            <dt>Nom</dt>
            <dd>{{ gift.ot }} {{ genderSymbol(gift.otGender) }}</dd>
            <dt>ID</dt>
            <dd class="mono">{{ gift.trainerId }}</dd>
            <dt>Langue</dt>
            <dd>{{ gift.languageName }}</dd>
          </dl>
        </section>

        <section v-if="gift.moveNames.length">
          <h4>Attaques</h4>
          <ul class="moves">
            <li v-for="m in gift.moveNames" :key="m">{{ m }}</li>
          </ul>
          <p v-if="gift.relearnNames.length" class="sv-help">Attaques de base : {{ gift.relearnNames.join(", ") }}</p>
        </section>
        <section v-else>
          <h4>Attaques</h4>
          <p class="sv-help">Celles apprises au niveau du Pokémon.</p>
        </section>

        <section>
          <h4>Rencontre <Tip :title="GIFT_TERMS.fateful.title" :text="GIFT_TERMS.fateful.text" /></h4>
          <dl>
            <dt>Lieu</dt>
            <dd>{{ gift.metLocationName ?? (p.metLocation ? `n°${p.metLocation}` : "—") }}</dd>
            <template v-if="gift.eggLocationName">
              <dt>Lieu de l'œuf</dt>
              <dd>{{ gift.eggLocationName }}</dd>
            </template>
            <dt>Date</dt>
            <dd>{{ date ?? "Le jour de la réception" }}</dd>
            <dt>Rencontre fatidique</dt>
            <dd>{{ p.fateful ? "Oui" : "Non" }}</dd>
          </dl>
        </section>

        <section v-if="gift.ribbonNames.length">
          <h4>Rubans</h4>
          <div class="chips">
            <span v-for="r in gift.ribbonNames" :key="r" class="ribbon"><Icon name="ribbon" :size="12" /> {{ r }}</span>
          </div>
        </section>
      </template>

      <section v-else-if="date">
        <dl>
          <dt>Date</dt>
          <dd>{{ date }}</dd>
        </dl>
      </section>

      <ul v-if="gift.notes.length" class="notes">
        <li v-for="(n, i) in gift.notes" :key="i"><Icon name="info" :size="14" /> {{ n }}</li>
      </ul>
      <p v-if="gift.compatible === false" class="refused">
        <Icon name="alert" :size="14" /> {{ gift.incompatibleReason }}
        <Tip :title="GIFT_TERMS.generation.title" :text="GIFT_TERMS.generation.text" />
      </p>
    </div>

    <footer class="actions">
      <button class="sv-btn solid big" :disabled="addDisabled" :title="addHint" @click="emit('add')">
        <Icon name="download" :size="16" /> Ajouter à la sauvegarde
      </button>
      <Tip :title="GIFT_TERMS.receive.title" :text="GIFT_TERMS.receive.text" />
      <div class="row">
        <button class="sv-btn" :disabled="busy" @click="emit('export')"><Icon name="save" :size="15" /> Enregistrer le fichier (.{{ gift.extension }})</button>
        <button v-if="lastAdded" class="sv-btn" @click="emit('edit')"><Icon name="pencil" :size="15" /> Ouvrir dans l'éditeur</button>
      </div>
    </footer>
  </aside>
</template>

<style scoped>
.details {
  display: flex;
  flex-direction: column;
  min-height: 0;
  padding: 16px;
  gap: 12px;
}

.head {
  display: flex;
  gap: 14px;
  align-items: center;
}

.art {
  position: relative;
  display: grid;
  place-items: center;
  width: 120px;
  height: 104px;
  flex-shrink: 0;
  border-radius: 16px;
  background: color-mix(in srgb, var(--gift-tone, var(--text)) 16%, transparent);
}

.egg-badge {
  position: absolute;
  right: 6px;
  bottom: 6px;
  display: grid;
  place-items: center;
  width: 24px;
  height: 24px;
  border-radius: 50%;
  background: var(--surface);
}

.meta {
  display: flex;
  flex-direction: column;
  gap: 5px;
  min-width: 0;
}

.meta h3 {
  margin: 0;
  font-size: 17px;
  line-height: 1.25;
  overflow-wrap: anywhere;
}

.meta small {
  color: var(--text-dim);
  font-weight: 600;
}

.badges {
  display: flex;
  flex-wrap: wrap;
  gap: 5px;
  align-items: center;
}

.fmt {
  padding: 2px 8px;
  border-radius: 6px;
  background: var(--gift-tone);
  color: #fff;
  font-size: 11px;
  font-weight: 800;
  letter-spacing: 0.05em;
}

.card-no,
.kind {
  font-size: 12px;
  font-weight: 700;
  color: var(--text-dim);
}

.f-pcd,
.f-pgt {
  --gift-tone: #3b82f6;
}

.f-pgf {
  --gift-tone: #475569;
}

.f-wc6 {
  --gift-tone: #db2777;
}

.f-wc7 {
  --gift-tone: #ea7a1a;
}

.scroll {
  display: flex;
  flex-direction: column;
  gap: 14px;
  min-height: 0;
  overflow-y: auto;
  padding-right: 4px;
}

h4 {
  display: flex;
  align-items: center;
  margin: 0 0 7px;
  color: var(--text-dim);
  font-size: 11px;
  font-weight: 700;
  letter-spacing: 0.12em;
  text-transform: uppercase;
}

.chips {
  display: flex;
  flex-wrap: wrap;
  gap: 5px;
}

.game,
.ribbon {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 3px 10px;
  border-radius: 999px;
  background: color-mix(in srgb, var(--text) 13%, transparent);
  font-size: 12px;
  font-weight: 700;
}

dl {
  display: grid;
  grid-template-columns: auto 1fr;
  gap: 5px 12px;
  margin: 0;
  font-size: 13px;
}

dt {
  display: flex;
  align-items: center;
  color: var(--text-dim);
}

dd {
  margin: 0;
  font-weight: 600;
  text-align: right;
  overflow-wrap: anywhere;
}

dd.gold {
  color: #ffd45c;
}

.mono {
  font-family: "Cascadia Mono", Consolas, monospace;
}

.moves,
.list {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 6px;
  margin: 0 0 6px;
  padding: 0;
  list-style: none;
}

.list {
  grid-template-columns: 1fr;
}

.moves li,
.list li {
  padding: 6px 10px;
  border-radius: 8px;
  background: color-mix(in srgb, var(--text) 12%, transparent);
  font-size: 13px;
  font-weight: 600;
}

.notes {
  display: flex;
  flex-direction: column;
  gap: 6px;
  margin: 0;
  padding: 0;
  list-style: none;
  color: var(--text-dim);
  font-size: 12.5px;
  line-height: 1.4;
}

.refused {
  margin: 0;
  padding: 10px 12px;
  border-radius: 10px;
  background: color-mix(in srgb, var(--danger) 16%, transparent);
  color: var(--text);
  font-size: 13px;
  line-height: 1.4;
}

.actions {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 8px;
  padding-top: 4px;
  border-top: 1px solid var(--border);
}

.actions .big {
  flex: 1;
  padding: 11px;
}

.actions .row {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  width: 100%;
}
</style>
