<script setup lang="ts">
import Tip from "../components/Tip.vue";
import { PLAY_TIPS } from "./glossary";

/** Bulle « i » pour un terme de PLAY_TIPS. */
const props = defineProps<{ term: keyof typeof PLAY_TIPS }>();
const entry = PLAY_TIPS[props.term];
</script>

<template>
  <Tip :title="entry.title" :text="entry.text" />
</template>
