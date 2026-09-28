"""JSON API for a separate ballot UI. Weeks and QB rosters are managed by admins."""

from django.conf import settings
from django.db import transaction
from django.db.models import Count, F, Q, Sum
from django.middleware.csrf import get_token
from django.shortcuts import get_object_or_404
from ninja import NinjaAPI, Schema
from ninja.security import django_auth

from .models import Ballot, PollWeek, Quarterback, Team, Vote

api = NinjaAPI(
    title="QB Poll API",
    auth=django_auth,
    openapi_url="/openapi.json" if settings.DEBUG else None,
    docs_url="/docs" if settings.DEBUG else None,
)


class TeamOut(Schema):
    id: int
    name: str
    abbreviation: str


class QuarterbackOut(Schema):
    id: int
    name: str
    team: TeamOut | None


class WeekOut(Schema):
    id: int
    season: int
    week: int


class BallotIn(Schema):
    # Ordered from #1 to #15; ranks are derived from list position.
    quarterback_ids: list[int]


class BallotEntry(Schema):
    rank: int
    quarterback: QuarterbackOut


class BallotOut(Schema):
    season: int
    week: int
    entries: list[BallotEntry]


class RankingEntry(Schema):
    rank: int
    points: int
    votes: int
    quarterback: QuarterbackOut


class RankingsOut(Schema):
    season: int
    week: int
    ballots: int
    rankings: list[RankingEntry]


class ErrorOut(Schema):
    detail: str


def qb_out(qb):
    return {
        "id": qb.pk,
        "name": qb.name,
        "team": (
            {
                "id": qb.team.pk,
                "name": qb.team.name,
                "abbreviation": qb.team.abbreviation,
            }
            if qb.team
            else None
        ),
    }


@api.get("/csrf", response=dict[str, str])
def csrf_token(request):
    """Issue the CSRF cookie/token for a browser client before ballot writes."""
    return {"csrf_token": get_token(request)}


@api.get("/teams", response=list[TeamOut])
def list_teams(request):
    get_token(request)
    return Team.objects.order_by("name")


@api.get("/quarterbacks", response={200: list[QuarterbackOut], 400: ErrorOut})
def list_quarterbacks(
    request, q: str = "", team_id: int | None = None, limit: int = 50, offset: int = 0
):
    """Case-insensitive substring search across QB name, team name and abbreviation."""
    if limit < 1 or limit > 100 or offset < 0:
        return api.create_response(
            request,
            {"detail": "limit must be 1-100 and offset nonnegative"},
            status=400,
        )
    qbs = Quarterback.objects.select_related("team").order_by("name", "id")
    if q.strip():
        qbs = qbs.filter(
            Q(name__icontains=q.strip())
            | Q(team__name__icontains=q.strip())
            | Q(team__abbreviation__icontains=q.strip())
        )
    if team_id is not None:
        qbs = qbs.filter(team_id=team_id)
    return [qb_out(qb) for qb in qbs[offset : offset + limit]]


@api.get("/weeks", response=list[WeekOut])
def list_weeks(request):
    get_token(request)
    return PollWeek.objects.all()


def get_week(season, week):
    return get_object_or_404(PollWeek, season=season, week=week)


def ballot_out(week, ballot):
    entries = []
    if ballot is not None:
        entries = [
            {"rank": vote.rank, "quarterback": qb_out(vote.quarterback)}
            for vote in ballot.votes.select_related("quarterback__team").all()
        ]
    return {"season": week.season, "week": week.week, "entries": entries}


@api.get("/weeks/{season}/{week}/ballot", response=BallotOut)
def my_ballot(request, season: int, week: int):
    poll_week = get_week(season, week)
    ballot = Ballot.objects.filter(poll_week=poll_week, voter=request.user).first()
    return ballot_out(poll_week, ballot)


@api.put(
    "/weeks/{season}/{week}/ballot",
    response={200: BallotOut, 400: ErrorOut},
)
def submit_ballot(request, season: int, week: int, payload: BallotIn):
    """Create or replace the caller's complete ballot atomically (never append votes)."""
    poll_week = get_week(season, week)
    ids = payload.quarterback_ids
    if len(ids) != 15 or len(set(ids)) != 15:
        return 400, {"detail": "A ballot must contain exactly 15 distinct quarterbacks"}
    if Quarterback.objects.filter(pk__in=ids).count() != 15:
        return 400, {"detail": "One or more quarterbacks do not exist"}
    with transaction.atomic():
        ballot, _ = Ballot.objects.get_or_create(
            poll_week=poll_week, voter=request.user
        )
        ballot.votes.all().delete()
        Vote.objects.bulk_create(
            [
                Vote(ballot=ballot, quarterback_id=qb_id, rank=rank)
                for rank, qb_id in enumerate(ids, start=1)
            ]
        )
    return ballot_out(poll_week, ballot)


@api.get("/weeks/{season}/{week}/rankings", response=RankingsOut)
def rankings(request, season: int, week: int):
    """AP-style 15..1 points; top ten by points, with deterministic tie order."""
    get_token(request)
    poll_week = get_week(season, week)
    scores = list(
        Vote.objects.filter(ballot__poll_week=poll_week)
        .values("quarterback_id")
        .annotate(points=Sum(16 - F("rank")), votes=Count("id"))
        .order_by("-points", "-votes", "quarterback__name", "quarterback_id")[:10]
    )
    qbs = Quarterback.objects.select_related("team").in_bulk(
        row["quarterback_id"] for row in scores
    )
    return {
        "season": season,
        "week": week,
        "ballots": Ballot.objects.filter(poll_week=poll_week).count(),
        "rankings": [
            {
                "rank": rank,
                "points": row["points"],
                "votes": row["votes"],
                "quarterback": qb_out(qbs[row["quarterback_id"]]),
            }
            for rank, row in enumerate(scores, start=1)
        ],
    }
