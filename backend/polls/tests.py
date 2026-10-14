from django.contrib.auth import get_user_model
from django.db import IntegrityError, transaction
from django.test import TestCase

from .models import Ballot, PollWeek, Quarterback, Team, Vote


class SeedDataTests(TestCase):
    def test_teams_and_quarterbacks_are_imported(self):
        self.assertEqual(Team.objects.count(), 32)
        self.assertEqual(Quarterback.objects.count(), 92)
        self.assertEqual(
            Quarterback.objects.get(name="Gardner Minshew II").team.name,
            "Arizona Cardinals",
        )
        self.assertEqual(
            Quarterback.objects.get(name="Bryce Young").team.abbreviation,
            "CAR",
        )


class PollModelTests(TestCase):
    def setUp(self):
        self.voter = get_user_model().objects.create_user(username="voter")
        self.week = PollWeek.objects.create(season=2026, week=1)
        self.team = Team.objects.create(name="Test Team", abbreviation="TST")
        self.qb = Quarterback.objects.create(name="Test Quarterback", team=self.team)
        self.other_qb = Quarterback.objects.create(name="Other Test Quarterback")
        self.ballot = Ballot.objects.create(poll_week=self.week, voter=self.voter)

    def test_ballot_records_ranked_qbs_for_voter_and_week(self):
        Vote.objects.create(ballot=self.ballot, quarterback=self.other_qb, rank=2)
        Vote.objects.create(ballot=self.ballot, quarterback=self.qb, rank=1)
        self.assertEqual(
            list(self.ballot.votes.values_list("rank", "quarterback_id")),
            [(1, self.qb.pk), (2, self.other_qb.pk)],
        )

    def test_one_ballot_per_voter_per_week(self):
        with self.assertRaises(IntegrityError), transaction.atomic():
            Ballot.objects.create(poll_week=self.week, voter=self.voter)
        next_week = PollWeek.objects.create(season=2026, week=2)
        Ballot.objects.create(poll_week=next_week, voter=self.voter)
        self.assertEqual(Ballot.objects.count(), 2)

    def test_rank_and_qb_cannot_repeat_on_ballot(self):
        Vote.objects.create(ballot=self.ballot, quarterback=self.qb, rank=1)
        with self.assertRaises(IntegrityError), transaction.atomic():
            Vote.objects.create(ballot=self.ballot, quarterback=self.other_qb, rank=1)
        with self.assertRaises(IntegrityError), transaction.atomic():
            Vote.objects.create(ballot=self.ballot, quarterback=self.qb, rank=2)

    def test_rank_and_week_must_be_positive(self):
        with self.assertRaises(IntegrityError), transaction.atomic():
            Vote.objects.create(ballot=self.ballot, quarterback=self.qb, rank=0)
        with self.assertRaises(IntegrityError), transaction.atomic():
            PollWeek.objects.create(season=2026, week=0)

    def test_same_week_number_in_different_seasons(self):
        PollWeek.objects.create(season=2027, week=1)
        with self.assertRaises(IntegrityError), transaction.atomic():
            PollWeek.objects.create(season=2026, week=1)

    def test_team_change_does_not_change_vote(self):
        vote = Vote.objects.create(ballot=self.ballot, quarterback=self.qb, rank=1)
        self.team.delete()
        vote.refresh_from_db()
        vote.quarterback.refresh_from_db()
        self.assertIsNone(vote.quarterback.team)
        self.assertEqual(vote.quarterback, self.qb)
