<script setup lang="ts">
import { computed } from "vue";
import Banner from "../../components/Banner.vue";
import Icon from "../../components/Icon.vue";
import Sprite from "../../components/Sprite.vue";
import Tip from "../../components/Tip.vue";
import { formatDate, isShiny, type GiftDetails } from "./api";
import FormatBadge from "./FormatBadge.vue";
import { giftTone } from "./terms";

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
      <div class="art" :style="giftTone(gift.formatLabel)">
        <Sprite v-if="gift.species" :id="gift.species" :shiny="shiny" :size="112" />
        <Icon v-else :name="gift.kind === 'item' ? 'bag' : 'gift'" :size="56" />
        <span v-if="gift.egg" class="egg-badge" title="Œuf"><Icon name="egg" :size="16" /></span>
      </div>
      <div class="meta">
        <div class="badges">
          <FormatBadge :label="gift.formatLabel" />
          <span v-if="gift.cardId" class="card-no">Carte n°{{ gift.cardId }}</span>
          <span class="kind">{{ gift.kindLabel }}</span>
        </div>
        <h3>{{ gift.title }}</h3>
        <small v-if="p">
          {{ gift.speciesName }}<template v-if="gift.formName"> ({{ gift.formName }})</template>
          · {{ p.level ? `N. ${p.level}` : "niveau au hasard" }}
          <template v-if="shiny"> · <span class="shiny"><Icon name="star" :size="12" /> Chromatique</span></template>
        </small>
      </div>
    </header>

    <div class="scroll">
      <section>
        <h4 class="sv-label">Jeux <Tip term="gifts.distribution" /></h4>
        <div class="sv-row chips">
          <span v-for="g in gift.games" :key="g.id" class="sv-chip">{{ g.name }}</span>
        </div>
      </section>

      <section v-if="gift.itemNames.length">
        <h4 class="sv-label">Objets</h4>
        <div class="sv-row chips">
          <span v-for="(n, i) in gift.itemNames" :key="i" class="sv-chip"><Icon name="bag" :size="12" /> {{ n }}</span>
        </div>
      </section>

      <template v-if="p">
        <section>
          <h4 class="sv-label">Pokémon</h4>
          <dl class="sv-dl">
            <dt>Espèce <Tip term="species" /></dt>
            <dd>{{ gift.speciesName }}<template v-if="gift.formName"> ({{ gift.formName }})</template></dd>
            <dt>Niveau <Tip term="level" /></dt>
            <dd>{{ p.level ? `N. ${p.level}` : "Au hasard" }}<template v-if="p.metLevel && p.metLevel !== p.level"> (rencontré N. {{ p.metLevel }})</template></dd>
            <template v-if="p.nickname">
              <dt>Surnom <Tip term="nickname" /></dt>
              <dd>{{ p.nickname }}</dd>
            </template>
            <dt>Œuf <Tip term="egg" /></dt>
            <dd>{{ p.egg ? "Oui" : "Non" }}</dd>
            <dt>Chromatique <Tip term="shinyLock" /></dt>
            <dd :class="{ shiny }">{{ gift.shinyLabel }}</dd>
            <dt>Sexe <Tip term="gender" /></dt>
            <dd>{{ gift.genderLabel }}</dd>
            <dt>Nature <Tip term="nature" /></dt>
            <dd>{{ gift.natureName }}</dd>
            <dt>Talent <Tip term="ability" /></dt>
            <dd>{{ gift.abilityLabel }}</dd>
            <dt>IV <Tip term="gifts.perfectIvs" /></dt>
            <dd>{{ gift.ivsLabel }}</dd>
            <dt>Objet tenu <Tip term="heldItem" /></dt>
            <dd>{{ gift.heldItemName ?? "Aucun" }}</dd>
            <dt>Ball <Tip term="ball" /></dt>
            <dd>{{ gift.ballName ?? "—" }}</dd>
          </dl>
        </section>

        <section>
          <h4 class="sv-label">Dresseur d'origine <Tip term="gifts.ot" /></h4>
          <dl class="sv-dl">
            <dt>Nom</dt>
            <dd>{{ gift.ot }} {{ genderSymbol(gift.otGender) }}</dd>
            <dt>ID <Tip term="tid" /></dt>
            <dd class="mono">{{ gift.trainerId }}</dd>
            <dt>Langue <Tip term="language" /></dt>
            <dd>{{ gift.languageName }}</dd>
          </dl>
        </section>

        <section>
          <h4 class="sv-label">Attaques <Tip term="moves" /></h4>
          <ul v-if="gift.moveNames.length" class="sv-moves">
            <li v-for="m in gift.moveNames" :key="m">{{ m }}</li>
          </ul>
          <p v-else class="sv-help">Celles apprises au niveau du Pokémon.</p>
          <p v-if="gift.relearnNames.length" class="sv-help relearn">
            Attaques de base <Tip term="relearn" /> : {{ gift.relearnNames.join(", ") }}
          </p>
        </section>

        <section>
          <h4 class="sv-label">Rencontre</h4>
          <dl class="sv-dl">
            <dt>Lieu de rencontre <Tip term="metLocation" /></dt>
            <dd>{{ gift.metLocationName ?? (p.metLocation ? `n°${p.metLocation}` : "—") }}</dd>
            <template v-if="gift.eggLocationName">
              <dt>Lieu de l'œuf <Tip term="eggLocation" /></dt>
              <dd>{{ gift.eggLocationName }}</dd>
            </template>
            <dt>Date <Tip term="metDate" /></dt>
            <dd>{{ date ?? "Le jour de la réception" }}</dd>
            <dt>Rencontre fatidique <Tip term="fateful" /></dt>
            <dd>{{ p.fateful ? "Oui" : "Non" }}</dd>
          </dl>
        </section>

        <section v-if="gift.ribbonNames.length">
          <h4 class="sv-label">Rubans <Tip term="ribbons" /></h4>
          <div class="sv-row chips">
            <span v-for="r in gift.ribbonNames" :key="r" class="sv-chip"><Icon name="ribbon" :size="12" /> {{ r }}</span>
          </div>
        </section>
      </template>

      <section v-else-if="date">
        <dl class="sv-dl">
          <dt>Date</dt>
          <dd>{{ date }}</dd>
        </dl>
      </section>

      <ul v-if="gift.notes.length" class="notes">
        <li v-for="(n, i) in gift.notes" :key="i"><Icon name="info" :size="14" /> {{ n }}</li>
      </ul>
      <Banner v-if="gift.compatible === false" tone="warn">
        {{ gift.incompatibleReason }}
        <Tip term="gifts.generation" />
      </Banner>
    </div>

    <footer class="actions">
      <button type="button" class="sv-btn solid big" :disabled="addDisabled" :title="addHint" @click="emit('add')">
        <Icon name="download" :size="16" /> Ajouter à la sauvegarde
      </button>
      <Tip term="gifts.receive" />
      <div class="sv-row row">
        <button type="button" class="sv-btn" :disabled="busy" @click="emit('export')">
          <Icon name="save" :size="15" /> Enregistrer le fichier (.{{ gift.extension }})
        </button>
        <button v-if="lastAdded" type="button" class="sv-btn" @click="emit('edit')"><Icon name="pencil" :size="15" /> Ouvrir dans l'éditeur</button>
      </div>
    </footer>
  </aside>
</template>

<style scoped>
.details {
  display: flex;
  flex-direction: column;
  min-height: 0;
  padding: var(--sp-4);
  gap: var(--sp-3);
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
  border-radius: var(--radius-card);
  background: color-mix(in srgb, var(--gift-tone) 16%, transparent);
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
  font-size: var(--fs-lg);
  line-height: 1.25;
  overflow-wrap: anywhere;
}

.meta small {
  color: var(--text-dim);
  font-weight: 600;
}

.shiny {
  color: var(--shiny);
}

.badges {
  display: flex;
  flex-wrap: wrap;
  gap: 5px;
  align-items: center;
}

.card-no,
.kind {
  font-size: var(--fs-sm);
  font-weight: 700;
  color: var(--text-dim);
}

.scroll {
  display: flex;
  flex-direction: column;
  gap: 14px;
  min-height: 0;
  overflow-y: auto;
  padding-right: var(--sp-1);
}

h4 {
  margin: 0 0 7px;
}

.chips {
  gap: 5px;
}

.sv-dl dd {
  overflow-wrap: anywhere;
}

.mono {
  font-family: "Cascadia Mono", Consolas, monospace;
}

.relearn {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  margin-top: 6px;
}

.notes {
  display: flex;
  flex-direction: column;
  gap: 6px;
  margin: 0;
  padding: 0;
  list-style: none;
  color: var(--text-dim);
  font-size: var(--fs-sm);
  line-height: 1.4;
}

.actions {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: var(--sp-2);
  padding-top: var(--sp-1);
  border-top: 1px solid var(--border);
}

.actions .big {
  flex: 1;
  padding: 11px;
}

.actions .row {
  width: 100%;
}
</style>
