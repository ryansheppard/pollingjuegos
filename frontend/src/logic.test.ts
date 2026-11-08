import { describe, expect, it } from 'vitest'
import type { BallotEntry, Quarterback, QuarterbackStats, RankBand, Rankings, Week } from './api.gen'
import { BALLOT_SIZE, badge, disagreements, isClosed, matchesSearch, saveHint, timeLeft } from './logic'

// Cases mirror desktop/src/screen/poll/logic.rs so the two clients can't drift apart unnoticed.

const NOW = Date.parse('2026-11-07T12:00:00Z')

function week(closesAt: string | null, isClosedFlag = false): Week {
  return { id: 1, season: 2026, week: 1, closes_at: closesAt, is_closed: isClosedFlag, is_default: false }
}

function qb(id: number): Quarterback {
  return { id, name: `QB ${id}`, team: null }
}

function stats(qbrRank: [number, RankBand] | null, qualified: boolean): QuarterbackStats {
  return {
    season: 2026,
    games: 8,
    dropbacks: 300,
    epa: 12.5,
    epa_per_dropback: -0.041,
    cpoe: 0,
    qbr: 61.27,
    qualified,
    qualified_count: 32,
    qbr_rank: qbrRank?.[0] ?? null,
    epa_rank: null,
    cpoe_rank: null,
    qbr_band: qbrRank?.[1] ?? null,
    epa_band: null,
    cpoe_band: null,
  }
}

describe('timeLeft', () => {
  it('formats the time until the deadline', () => {
    expect(timeLeft(week('2026-11-09T15:05:30Z'), NOW)).toBe('2d 3h 05m')
    expect(timeLeft(week('2026-11-07T13:09:00Z'), NOW)).toBe('1h 09m')
    expect(timeLeft(week('2026-11-07T12:00:59Z'), NOW)).toBe('0m')
    expect(timeLeft(week('2026-11-07T11:00:00Z'), NOW)).toBe('Closed')
    expect(timeLeft(week(null), NOW)).toBe('Open')
  })
})

describe('isClosed', () => {
  it('closes once the deadline passes even if the server flag is stale', () => {
    expect(isClosed(week('2026-11-07T11:59:59Z'), NOW)).toBe(true)
    expect(isClosed(week('2026-11-07T12:00:01Z'), NOW)).toBe(false)
    expect(isClosed(week(null, true), NOW)).toBe(true)
    expect(isClosed(week(null), NOW)).toBe(false)
  })
})

describe('disagreements', () => {
  it('ranks the biggest gaps first, treating unranked picks as just outside the rankings', () => {
    const saved: BallotEntry[] = Array.from({ length: BALLOT_SIZE }, (_, i) => ({ rank: i + 1, quarterback: qb(i + 1) }))
    // The crowd agrees on QB 1, swaps QBs 2 and 3, and puts QB 15 at #4.
    const crowd = [[1, 1], [3, 2], [2, 3], [15, 4]] as const
    const rankings: Rankings = {
      season: 2026,
      week: 1,
      ballots: 3,
      rankings: crowd.map(([id, rank]) => ({ rank, points: 0, votes: 0, previous_rank: null, quarterback: qb(id) })),
    }
    const got = disagreements(saved, rankings).map((row) => [row.quarterback.id, row.mine, row.crowd])
    // QB 15 is 11 off; unranked QBs 4 and 5 are 7 and 6 from the "just outside" #11.
    expect(got).toEqual([[15, 15, 4], [4, 4, undefined], [5, 5, undefined]])
  })
})

describe('badge', () => {
  const summary = (b: ReturnType<typeof badge>) => [b.text, b.rank, b.tone]

  it('shows the crowd rank', () => {
    expect(summary(badge('crowd', 3, null))).toEqual(['Crowd #3', '', ''])
    expect(summary(badge('crowd', undefined, null))).toEqual(['Unranked', '', 'unranked'])
  })

  it('shows the stat, league rank and band', () => {
    expect(summary(badge('qbr', undefined, stats([9, 'mid'], true)))).toEqual(['QBR 61.3', '#9', 'rank-mid'])
    expect(summary(badge('cpoe', undefined, stats(null, true)))).toEqual(['CPOE 0.0', '', ''])
  })

  it('marks QBs with too few dropbacks as not qualified', () => {
    const b = badge('epa', undefined, stats(null, false))
    expect(summary(b)).toEqual(['EPA −0.04', 'n/q', ''])
    expect(b.title).toContain('too few dropbacks')
  })

  it('handles QBs with no stats this season', () => {
    expect(summary(badge('qbr', undefined, null))).toEqual(['QBR —', '', 'unranked'])
  })
})

describe('matchesSearch', () => {
  const allen: Quarterback = { id: 1, name: 'Josh Allen', team: { id: 1, name: 'Buffalo Bills', abbreviation: 'BUF' } }

  it('requires every term across name and team', () => {
    expect(matchesSearch(allen, 'bills ALLEN')).toBe(true)
    expect(matchesSearch(allen, 'buf')).toBe(true)
    expect(matchesSearch(allen, '  ')).toBe(true)
    expect(matchesSearch(allen, 'allen chiefs')).toBe(false)
  })

  it('matches QBs without a team by name only', () => {
    expect(matchesSearch(qb(7), 'qb 7')).toBe(true)
    expect(matchesSearch(qb(7), 'bills')).toBe(false)
  })
})

describe('saveHint', () => {
  it('counts down the remaining picks', () => {
    expect(saveHint(0, true, false)).toBe(`${BALLOT_SIZE} more picks to complete your ballot.`)
    expect(saveHint(BALLOT_SIZE - 1, true, false)).toBe('1 more pick to complete your ballot.')
    expect(saveHint(BALLOT_SIZE - 2, true, true)).toBe('Unsaved changes. 2 more picks to complete your ballot.')
  })

  it('reflects whether a full ballot is saved', () => {
    expect(saveHint(BALLOT_SIZE, false, true)).toBe('Ballot saved.')
    expect(saveHint(BALLOT_SIZE, true, true)).toBe('Unsaved changes. Save to update your ballot.')
    expect(saveHint(BALLOT_SIZE, true, false)).toBe('Lineup complete. Ready to save.')
  })
})
