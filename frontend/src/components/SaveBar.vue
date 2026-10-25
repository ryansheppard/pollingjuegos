<script setup lang="ts">
import { computed } from 'vue'

const props = defineProps<{ count: number, closed: boolean, saving: boolean, dirty: boolean, hasSaved: boolean }>()
defineEmits<{ add: [], save: [] }>()

// oxlint-disable-next-line no-unused-vars -- Used in the Vue template.
const hint = computed(() => {
  const remaining = 15 - props.count
  const unsaved = props.dirty && props.hasSaved ? 'Unsaved changes. ' : ''
  if (remaining) return `${unsaved}${remaining} more ${remaining === 1 ? 'pick' : 'picks'} to complete your ballot.`
  if (!props.dirty) return 'Ballot saved.'
  return props.hasSaved ? 'Unsaved changes. Save to update your ballot.' : 'Lineup complete. Ready to save.'
})
</script>

<template>
  <div class="save-bar">
    <div class="ticks" role="progressbar" :aria-valuenow="count" aria-valuemin="0" aria-valuemax="15" aria-label="Ballot picks selected">
      <span v-for="n in 15" :key="n" :class="{ filled: n <= count }"></span>
    </div>
    <div class="save-row">
      <p class="save-hint">{{ hint }}</p>
      <div class="save-actions">
        <button v-if="!closed && count < 15" type="button" class="secondary-button add-jump" @click="$emit('add')">
          <svg aria-hidden="true" viewBox="0 0 24 24"><path d="M12 5v14M5 12h14" /></svg> Add QB
        </button>
        <button type="button" class="save-button" :disabled="count !== 15 || !dirty || saving || closed" @click="$emit('save')">
          {{ saving ? 'Saving…' : closed ? 'Voting closed' : count === 15 && !dirty ? 'Saved' : 'Save ballot' }}
          <svg aria-hidden="true" viewBox="0 0 24 24"><path d="M5 12h14M13 6l6 6-6 6" /></svg>
        </button>
      </div>
    </div>
  </div>
</template>
