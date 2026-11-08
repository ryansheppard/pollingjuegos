<script setup lang="ts">
import { computed } from 'vue'
import { BALLOT_SIZE, saveHint } from '../logic'

const props = defineProps<{ count: number, closed: boolean, saving: boolean, dirty: boolean, hasSaved: boolean }>()
defineEmits<{ add: [], save: [] }>()

const hint = computed(() => saveHint(props.count, props.dirty, props.hasSaved))
</script>

<template>
  <div class="save-bar">
    <div class="ticks" role="progressbar" :aria-valuenow="count" aria-valuemin="0" :aria-valuemax="BALLOT_SIZE" aria-label="Ballot picks selected">
      <span v-for="n in BALLOT_SIZE" :key="n" :class="{ filled: n <= count }"></span>
    </div>
    <div class="save-row">
      <p class="save-hint">{{ hint }}</p>
      <div class="save-actions">
        <button v-if="!closed && count < BALLOT_SIZE" type="button" class="secondary-button add-jump" @click="$emit('add')">
          <svg aria-hidden="true" viewBox="0 0 24 24"><path d="M12 5v14M5 12h14" /></svg> Add QB
        </button>
        <button type="button" class="save-button" :disabled="count !== BALLOT_SIZE || !dirty || saving || closed" @click="$emit('save')">
          {{ saving ? 'Saving…' : closed ? 'Voting closed' : count === BALLOT_SIZE && !dirty ? 'Saved' : 'Save ballot' }}
          <svg aria-hidden="true" viewBox="0 0 24 24"><path d="M5 12h14M13 6l6 6-6 6" /></svg>
        </button>
      </div>
    </div>
  </div>
</template>
