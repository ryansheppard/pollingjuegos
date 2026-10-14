from django.conf import settings
from django.core.validators import MinValueValidator
from django.db import models
from django.db.models import Q
from django.utils import timezone


class ApprovedDiscordUser(models.Model):
    objects = models.Manager()
    # Discord snowflake ID, not the mutable display name.
    discord_id = models.CharField(max_length=20, unique=True)
    enabled = models.BooleanField(default=True)
    note = models.CharField(max_length=100, blank=True)

    def __str__(self) -> str:
        return str(self.discord_id)


class Team(models.Model):
    objects = models.Manager()
    name = models.CharField(max_length=100, unique=True)
    abbreviation = models.CharField(max_length=3, unique=True)

    def __str__(self) -> str:
        return str(self.name)


class Quarterback(models.Model):
    objects = models.Manager()
    name = models.CharField(max_length=100)
    # Current team only; existing votes remain tied to the QB after a trade.
    team = models.ForeignKey(
        Team,
        on_delete=models.SET_NULL,
        null=True,
        blank=True,
        related_name="quarterbacks",
    )

    def __str__(self) -> str:
        return str(self.name)


class PollWeek(models.Model):
    objects = models.Manager()
    season = models.PositiveIntegerField(validators=[MinValueValidator(1)])
    week = models.PositiveIntegerField(validators=[MinValueValidator(1)])
    closes_at = models.DateTimeField(
        null=True,
        blank=True,
        help_text="Voting closes at this time (UTC in the API). Leave blank for no deadline.",
    )

    @property
    def is_closed(self):
        return self.closes_at is not None and timezone.now() >= self.closes_at

    class Meta:
        constraints = [  # noqa: RUF012 - Django Meta expects a sequence of constraints
            models.UniqueConstraint(fields=["season", "week"], name="unique_poll_week"),
            models.CheckConstraint(
                condition=Q(season__gte=1), name="poll_season_positive"
            ),
            models.CheckConstraint(condition=Q(week__gte=1), name="poll_week_positive"),
        ]
        ordering = ["-season", "-week"]  # noqa: RUF012 - Django Meta option

    def __str__(self):
        return f"{self.season} week {self.week}"


class Ballot(models.Model):
    objects = models.Manager()
    poll_week = models.ForeignKey(
        PollWeek, on_delete=models.CASCADE, related_name="ballots"
    )
    voter = models.ForeignKey(
        settings.AUTH_USER_MODEL, on_delete=models.PROTECT, related_name="qb_ballots"
    )

    class Meta:
        constraints = [  # noqa: RUF012 - Django Meta expects a sequence of constraints
            models.UniqueConstraint(
                fields=["poll_week", "voter"], name="unique_weekly_ballot"
            )
        ]

    def __str__(self):
        return f"{self.voter} — {self.poll_week}"


class Vote(models.Model):
    objects = models.Manager()
    ballot = models.ForeignKey(Ballot, on_delete=models.CASCADE, related_name="votes")
    quarterback = models.ForeignKey(
        Quarterback, on_delete=models.PROTECT, related_name="votes"
    )
    rank = models.PositiveIntegerField(validators=[MinValueValidator(1)])

    class Meta:
        constraints = [  # noqa: RUF012 - Django Meta expects a sequence of constraints
            models.UniqueConstraint(
                fields=["ballot", "rank"], name="unique_ballot_rank"
            ),
            models.UniqueConstraint(
                fields=["ballot", "quarterback"], name="unique_ballot_qb"
            ),
            models.CheckConstraint(condition=Q(rank__gte=1), name="vote_rank_positive"),
        ]
        ordering = ["rank"]  # noqa: RUF012 - Django Meta option

    def __str__(self):
        return f"{self.ballot}: #{self.rank} {self.quarterback}"
