import type { Quarterback, QuarterbackStats, RankBand, Week } from './api.gen'

export const BALLOT_SIZE = 15

export type Lens = 'crowd' | 'qbr' | 'epa' | 'cpoe'
export const lenses: { key: Lens, label: string }[] = [
  { key: 'crowd', label: 'Crowd' },
  { key: 'qbr', label: 'QBR' },
  { key: 'epa', label: 'EPA' },
  { key: 'cpoe', label: 'CPOE' },
]

/** The server only flips `is_closed` when the list is fetched, so a deadline can pass while the page is open. */
export function isClosed(week: Week, now: number): boolean {
  return week.is_closed || (week.closes_at !== null && now >= Date.parse(week.closes_at))
}

export function timeLeft(week: Week, now: number): string {
  if (isClosed(week, now)) return 'Closed'
  if (!week.closes_at) return 'Open'
  const minutes = Math.max(0, Math.floor((Date.parse(week.closes_at) - now) / 60_000))
  const d = Math.floor(minutes / 1440), h = Math.floor((minutes % 1440) / 60), m = minutes % 60
  return d ? `${d}d ${h}h ${String(m).padStart(2, '0')}m` : h ? `${h}h ${String(m).padStart(2, '0')}m` : `${m}m`
}

/** Every whitespace-separated term must appear in the name, team name or abbreviation. */
export function matchesSearch(qb: Quarterback, search: string): boolean {
  const haystack = `${qb.name} ${qb.team?.name ?? ''} ${qb.team?.abbreviation ?? ''}`.toLowerCase()
  return search.trim().toLowerCase().split(/\s+/).every((term) => haystack.includes(term))
}

function signed(value: number, digits: number) {
  return `${value > 0 ? '+' : value < 0 ? '−' : ''}${Math.abs(value).toFixed(digits)}`
}

export type StatLens = Exclude<Lens, 'crowd'>
export interface Stat { key: StatLens, label: string, title: string, value: string }

export function statLine(stats: QuarterbackStats): Stat[] {
  return [
    { key: 'qbr', label: 'QBR', title: 'ESPN Total QBR', value: stats.qbr === null ? '—' : stats.qbr.toFixed(1) },
    { key: 'epa', label: 'EPA', title: `EPA per dropback (${stats.epa === null ? '—' : signed(stats.epa, 1)} total over ${stats.dropbacks} dropbacks)`, value: stats.epa_per_dropback === null ? '—' : signed(stats.epa_per_dropback, 2) },
    { key: 'cpoe', label: 'CPOE', title: 'Completion % over expected', value: stats.cpoe === null ? '—' : signed(stats.cpoe, 1) },
  ]
}

/** CSS classes for league-rank bands; '' is the plain style. */
export type Tone = '' | 'unranked' | 'rank-top' | 'rank-mid' | 'rank-low'
const bandTones: Record<RankBand, Tone> = { top: 'rank-top', mid: 'rank-mid', low: 'rank-low' }

export interface Badge { text: string, rank: string, tone: Tone, title: string | undefined }

export function badge(lens: Lens, crowdRank: number | undefined, stats: QuarterbackStats | null | undefined): Badge {
  if (lens === 'crowd') {
    return { text: crowdRank ? `Crowd #${crowdRank}` : 'Unranked', rank: '', tone: crowdRank ? '' : 'unranked', title: undefined }
  }
  const label = lenses.find((option) => option.key === lens)!.label
  if (!stats) return { text: `${label} —`, rank: '', tone: 'unranked', title: 'No stats this season' }
  const stat = statLine(stats).find((entry) => entry.key === lens)!
  const rank = stats[`${lens}_rank`]
  const band = stats[`${lens}_band`]
  return {
    text: `${stat.label} ${stat.value}`,
    rank: rank !== null ? `#${rank}` : stats.qualified ? '' : 'n/q',
    tone: band ? bandTones[band] : stat.value === '—' ? 'unranked' : '',
    title: stats.qualified ? stat.title : `${stat.title} · too few dropbacks to rank`,
  }
}

export function saveHint(count: number, dirty: boolean, hasSaved: boolean): string {
  const remaining = Math.max(0, BALLOT_SIZE - count)
  const unsaved = dirty && hasSaved ? 'Unsaved changes. ' : ''
  if (remaining) return `${unsaved}${remaining} more ${remaining === 1 ? 'pick' : 'picks'} to complete your ballot.`
  if (!dirty) return 'Ballot saved.'
  return hasSaved ? 'Unsaved changes. Save to update your ballot.' : 'Lineup complete. Ready to save.'
}
