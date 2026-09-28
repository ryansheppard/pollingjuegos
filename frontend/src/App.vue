<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { ApiError, loadQuarterbacks, request, saveBallot, type Ballot, type Quarterback, type Rankings, type Week } from './api'

const weeks = ref<Week[]>([])
const selected = ref('')
const catalog = ref<Quarterback[]>([])
const ballot = ref<Quarterback[]>([])
const rankings = ref<Rankings | null>(null)
const search = ref('')
const signedIn = ref(false)
const loading = ref(true)
const saving = ref(false)
const error = ref('')
const message = ref('')
let loadId = 0

const week = computed(() => weeks.value.find((w) => `${w.season}/${w.week}` === selected.value))
// oxlint-disable-next-line no-unused-vars -- Used in the Vue template.
const available = computed(() => {
  const selectedIds = new Set(ballot.value.map((qb) => qb.id))
  const query = search.value.trim().toLowerCase()
  return catalog.value.filter((qb) => !selectedIds.has(qb.id) &&
    `${qb.name} ${qb.team?.name ?? ''} ${qb.team?.abbreviation ?? ''}`.toLowerCase().includes(query))
})

// oxlint-disable-next-line no-unused-vars -- Used in the Vue template.
function label(qb: Quarterback) {
  return `${qb.name}${qb.team ? ` (${qb.team.abbreviation})` : ''}`
}

// oxlint-disable-next-line no-unused-vars -- Used in the Vue template.
function move(index: number, direction: number) {
  const entries = [...ballot.value]
  ;[entries[index], entries[index + direction]] = [entries[index + direction]!, entries[index]!]
  ballot.value = entries
  message.value = ''
}

async function loadWeek() {
  const current = week.value
  const id = ++loadId
  rankings.value = null
  ballot.value = []
  signedIn.value = false
  error.value = ''
  message.value = ''
  if (!current) return
  loading.value = true
  try {
    const base = `/weeks/${current.season}/${current.week}`
    const [results, mine] = await Promise.all([
      request<Rankings>(`${base}/rankings`),
      request<Ballot>(`${base}/ballot`).catch((err: unknown) => {
        if (err instanceof ApiError && err.status === 401) return null
        throw err
      }),
    ])
    if (id !== loadId) return
    rankings.value = results
    signedIn.value = mine !== null
    ballot.value = mine?.entries.map((entry) => entry.quarterback) ?? []
  } catch (err) {
    if (id === loadId) error.value = err instanceof Error ? err.message : 'Could not load this week.'
  } finally {
    if (id === loadId) loading.value = false
  }
}

// oxlint-disable-next-line no-unused-vars -- Used in the Vue template.
async function submit() {
  const current = week.value
  if (!current || ballot.value.length !== 15 || saving.value) return
  saving.value = true
  error.value = ''
  message.value = ''
  try {
    const result = await saveBallot(current, ballot.value.map((qb) => qb.id))
    ballot.value = result.entries.map((entry) => entry.quarterback)
    message.value = 'Ballot saved.'
    try {
      rankings.value = await request<Rankings>(`/weeks/${current.season}/${current.week}/rankings`)
    } catch {
      error.value = 'Ballot saved, but rankings could not be refreshed. Reload to try again.'
    }
  } catch (err) {
    error.value = err instanceof Error ? err.message : 'Could not save ballot.'
  } finally {
    saving.value = false
  }
}

watch(selected, loadWeek)
onMounted(async () => {
  try {
    const [weekList, qbs] = await Promise.all([request<Week[]>('/weeks'), loadQuarterbacks()])
    catalog.value = qbs
    weeks.value = weekList
    if (weekList.length) selected.value = `${weekList[0]!.season}/${weekList[0]!.week}`
    else loading.value = false
  } catch (err) {
    error.value = err instanceof Error ? err.message : 'Could not load poll data.'
    loading.value = false
  }
})
</script>

<template>
  <main>
    <header>
      <h1>PollingJuegos</h1>
      <a v-if="!signedIn" href="/accounts/discord/login/">Sign in with Discord to vote</a>
    </header>
    <label for="week">Week: </label>
    <select id="week" v-model="selected" :disabled="loading || saving || !weeks.length">
      <option v-for="w in weeks" :key="`${w.season}/${w.week}`" :value="`${w.season}/${w.week}`">
        {{ w.season }} week {{ w.week }}
      </option>
    </select>
    <p v-if="error" class="error" role="alert">{{ error }}</p>
    <p v-if="message" class="success" role="status">{{ message }}</p>
    <p v-if="loading">Loading…</p>
    <p v-else-if="!weeks.length">No weeks have been created yet.</p>
    <div v-else class="grid">
      <section>
        <h2>Rankings</h2>
        <p class="muted">{{ rankings?.ballots ?? 0 }} ballots</p>
        <ol v-if="rankings?.rankings.length">
          <li v-for="entry in rankings.rankings" :key="entry.quarterback.id">
            {{ label(entry.quarterback) }} — {{ entry.points }} points ({{ entry.votes }} votes)
          </li>
        </ol>
        <p v-else>No votes yet.</p>
      </section>
      <section>
        <h2>Your ballot ({{ ballot.length }}/15)</h2>
        <p v-if="!signedIn" class="muted">Sign in to rank quarterbacks.</p>
        <template v-else>
          <ol>
            <li v-for="(qb, index) in ballot" :key="qb.id">
              <div class="row">
                <span>{{ label(qb) }}</span>
                <div class="controls">
                  <button type="button" :disabled="saving || index === 0" :aria-label="`Move ${qb.name} up`" @click="move(index, -1)">↑</button>
                  <button type="button" :disabled="saving || index === ballot.length - 1" :aria-label="`Move ${qb.name} down`" @click="move(index, 1)">↓</button>
                  <button type="button" :disabled="saving" :aria-label="`Remove ${qb.name}`" @click="ballot.splice(index, 1); message = ''">Remove</button>
                </div>
              </div>
            </li>
          </ol>
          <button type="button" :disabled="ballot.length !== 15 || saving" @click="submit">
            {{ saving ? 'Saving…' : 'Save ballot' }}
          </button>
        </template>
      </section>
      <section v-if="signedIn">
        <h2>Add quarterbacks</h2>
        <label for="search">Search by name or team</label>
        <input id="search" v-model="search" type="search" placeholder="Search quarterbacks" />
        <p v-if="!available.length" class="muted">No matching quarterbacks.</p>
        <ul v-else>
          <li v-for="qb in available" :key="qb.id">
            <div class="row">
              <span>{{ label(qb) }}</span>
              <button type="button" :disabled="ballot.length >= 15 || saving" :aria-label="`Add ${qb.name}`" @click="ballot.push(qb); message = ''">Add</button>
            </div>
          </li>
        </ul>
      </section>
    </div>
  </main>
</template>
