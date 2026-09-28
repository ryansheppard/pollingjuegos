import json

from allauth.socialaccount.models import SocialAccount
from django.contrib.auth import get_user_model
from django.test import Client, TestCase

from .models import ApprovedDiscordUser, Ballot, PollWeek, Quarterback, Vote


class PollAPITests(TestCase):
    def setUp(self):
        self.week = PollWeek.objects.create(season=2026, week=1)
        self.qbs = list(Quarterback.objects.order_by("id")[:16])
        self.user = self.make_user("voter", "1234")
        self.other = self.make_user("other", "5678")
        self.client = Client(enforce_csrf_checks=True)
        self.client.force_login(self.user, backend="config.auth.DiscordOnlyBackend")
        # Browser bootstrap returns both the cookie and the matching token.
        self.csrf_token = self.client.get("/api/csrf").json()["csrf_token"]

    @staticmethod
    def make_user(username, discord_id):
        user = get_user_model().objects.create_user(username=username)
        ApprovedDiscordUser.objects.create(discord_id=discord_id)
        SocialAccount.objects.create(user=user, provider="discord", uid=discord_id)  # ty: ignore[unresolved-attribute]
        return user

    def put_ballot(self, ids, season=2026, week=1):
        return self.client.put(
            f"/api/weeks/{season}/{week}/ballot",
            data=json.dumps({"quarterback_ids": ids}),
            content_type="application/json",
            HTTP_X_CSRFTOKEN=self.csrf_token,
        )

    def test_schema_and_docs_follow_debug_setting(self):
        from django.conf import settings

        expected_status = 200 if settings.DEBUG else 404
        self.assertEqual(
            self.client.get("/api/openapi.json").status_code, expected_status
        )
        self.assertEqual(self.client.get("/api/docs").status_code, expected_status)

    def test_catalog_search_and_weeks(self):
        self.assertEqual(self.client.get("/api/weeks").json()[0]["week"], 1)
        qb = Quarterback.objects.get(name="Patrick Mahomes")
        for query in ("mahOm", qb.team.name[:6].lower(), qb.team.abbreviation.lower()):
            ids = [
                row["id"]
                for row in self.client.get("/api/quarterbacks", {"q": query}).json()
            ]
            self.assertIn(qb.id, ids)
        self.assertIn(
            qb.team.id, [team["id"] for team in self.client.get("/api/teams").json()]
        )
        result = self.client.get(
            "/api/quarterbacks", {"team_id": qb.team_id, "limit": 1, "offset": 0}
        )
        self.assertEqual(len(result.json()), 1)
        self.assertEqual(result.json()[0]["team"]["id"], qb.team_id)
        self.assertEqual(
            self.client.get("/api/quarterbacks", {"limit": 101}).status_code, 400
        )

    def test_ballot_validation_replacement_and_isolation(self):
        ids = [qb.id for qb in self.qbs[:15]]
        url = "/api/weeks/2026/1/ballot"
        self.assertEqual(self.client.get(url).json()["entries"], [])
        for bad in (ids[:14], ids[:14] + ids[:1], ids[:14] + [999999]):
            self.assertEqual(self.put_ballot(bad).status_code, 400)
        self.assertEqual(Ballot.objects.count(), 0)
        response = self.put_ballot(ids)
        self.assertEqual(response.status_code, 200, response.content)
        self.assertEqual(
            [e["quarterback"]["id"] for e in response.json()["entries"]], ids
        )
        replacement = ids[1:] + [self.qbs[15].id]
        self.assertEqual(self.put_ballot(replacement).status_code, 200)
        self.assertEqual(Ballot.objects.count(), 1)
        self.assertEqual(Vote.objects.count(), 15)
        self.assertEqual(
            self.client.get(url).json()["entries"][0]["quarterback"]["id"],
            replacement[0],
        )
        self.client.force_login(self.other, backend="config.auth.DiscordOnlyBackend")
        self.assertEqual(self.client.get(url).json()["entries"], [])
        self.assertEqual(self.put_ballot(ids).status_code, 200)
        self.assertEqual(Ballot.objects.count(), 2)
        self.assertEqual(self.client.get("/api/weeks/2026/2/rankings").status_code, 404)

    def test_rankings_count_points_and_top_ten(self):
        ids = [qb.id for qb in self.qbs[:15]]
        self.put_ballot(ids)
        self.client.force_login(self.other, backend="config.auth.DiscordOnlyBackend")
        self.put_ballot(ids[1:] + [self.qbs[15].id])
        result = self.client.get("/api/weeks/2026/1/rankings").json()
        self.assertEqual(result["ballots"], 2)
        self.assertEqual(len(result["rankings"]), 10)
        self.assertEqual(result["rankings"][0]["quarterback"]["id"], ids[1])
        self.assertEqual(result["rankings"][0]["points"], 29)
        self.assertEqual(result["rankings"][0]["votes"], 2)
        self.assertEqual(
            [row["rank"] for row in result["rankings"]], list(range(1, 11))
        )

    def test_auth_and_csrf(self):
        ids = [qb.id for qb in self.qbs[:15]]
        url = "/api/weeks/2026/1/ballot"
        response = self.client.put(
            url, json.dumps({"quarterback_ids": ids}), content_type="application/json"
        )
        self.assertEqual(response.status_code, 403)
        self.assertEqual(Ballot.objects.count(), 0)
        self.client.logout()
        self.assertEqual(self.client.get(url).status_code, 401)
        self.assertEqual(
            self.put_ballot(ids).status_code, 403
        )  # logged-out CSRF cookie cleared
        self.assertEqual(self.client.get("/api/weeks/2026/1/rankings").status_code, 200)
