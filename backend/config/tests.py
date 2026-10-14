from allauth.core.context import request_context
from allauth.socialaccount.internal.flows.login import complete_login
from allauth.socialaccount.models import SocialAccount, SocialLogin
from django.conf import settings
from django.contrib.auth import get_user_model
from django.contrib.auth.models import AnonymousUser
from django.contrib.messages.storage.fallback import FallbackStorage
from django.contrib.sessions.middleware import SessionMiddleware
from django.test import RequestFactory, TestCase, override_settings
from django.urls import reverse

from polls.models import ApprovedDiscordUser

from .social import DiscordAllowlistAdapter


# Admin templates reference static assets; tests do not run collectstatic.
@override_settings(
    STORAGES={
        **settings.STORAGES,
        "staticfiles": {
            "BACKEND": "django.contrib.staticfiles.storage.StaticFilesStorage"
        },
    }
)
class DiscordLoginTests(TestCase):
    def social_login(self, uid, provider="discord"):
        return SocialLogin(
            user=get_user_model()(username=f"discord_{uid}"),
            account=SocialAccount(provider=provider, uid=uid),
        )

    def request(self):
        request = RequestFactory().get("/accounts/discord/login/callback/")
        SessionMiddleware(lambda _req: None).process_request(request)
        request._messages = FallbackStorage(request)
        request.user = AnonymousUser()
        return request

    def test_discord_routes_are_available_without_password_signup(self):
        self.assertEqual(reverse("discord_login"), "/accounts/discord/login/")
        self.assertEqual(
            reverse("discord_callback"), "/accounts/discord/login/callback/"
        )
        self.assertNotIn(
            b"password", self.client.get("/accounts/login/").content.lower()
        )

    def test_admin_login_shows_discord_instead_of_password(self):
        response = self.client.get("/admin/login/?next=/admin/")
        self.assertEqual(response.status_code, 200)
        self.assertContains(response, "Sign in with Discord")
        self.assertContains(response, "/accounts/discord/login/?next=%2Fadmin%2F")
        self.assertNotContains(response, 'type="password"')
        self.assertContains(
            self.client.get("/accounts/discord/login/?next=%2Fadmin%2F"),
            "Sign In Via Discord",
        )

    def test_unapproved_id_rejected_before_user_creation(self):
        request = self.request()
        response = complete_login(request, self.social_login("123"))
        self.assertEqual(response.status_code, 403)
        self.assertFalse(
            get_user_model().objects.filter(username="discord_123").exists()
        )
        self.assertNotIn("_auth_user_id", request.session)

    def test_approved_id_can_create_account_and_log_in(self):
        ApprovedDiscordUser.objects.create(discord_id="123")
        request = self.request()
        with request_context(request):
            response = complete_login(request, self.social_login("123"))
        self.assertEqual(response.status_code, 302)
        user = get_user_model().objects.get(username="discord_123")
        self.assertEqual(request.session["_auth_user_id"], str(user.pk))
        self.assertTrue(
            SocialAccount.objects.filter(  # ty: ignore[unresolved-attribute]
                user=user, provider="discord", uid="123"
            ).exists()
        )

    def test_approved_existing_account_can_log_in(self):
        user = get_user_model().objects.create_user(username="existing")
        SocialAccount.objects.create(  # ty: ignore[unresolved-attribute]
            user=user, provider="discord", uid="123"
        )
        ApprovedDiscordUser.objects.create(discord_id="123")
        request = self.request()
        with request_context(request):
            response = complete_login(request, self.social_login("123"))
        self.assertEqual(response.status_code, 302)
        self.assertEqual(request.session["_auth_user_id"], str(user.pk))

    def test_approval_is_exact_id_and_can_be_revoked(self):
        approved = ApprovedDiscordUser.objects.create(discord_id="123")
        adapter = DiscordAllowlistAdapter()
        self.assertTrue(
            adapter.is_open_for_signup(self.request(), self.social_login("123"))
        )
        self.assertFalse(
            adapter.is_open_for_signup(self.request(), self.social_login("124"))
        )
        self.assertFalse(
            adapter.is_open_for_signup(
                self.request(), self.social_login("123", "github")
            )
        )
        approved.enabled = False
        approved.save()
        response = complete_login(self.request(), self.social_login("123"))
        self.assertEqual(response.status_code, 403)

    def test_password_login_cannot_bypass_approval(self):
        user = get_user_model().objects.create_user(username="local", password="secret")
        ApprovedDiscordUser.objects.create(discord_id="123")
        self.assertFalse(self.client.login(username="local", password="secret"))
        self.assertTrue(user.is_active)

    def test_revoked_user_loses_existing_session(self):
        self.assertEqual(self.client.get("/").status_code, 302)
        response = self.client.get("/auth/frontend/")
        self.assertRedirects(
            response, "/accounts/discord/login/?next=/", fetch_redirect_response=False
        )
        user = get_user_model().objects.create_superuser(
            username="admin", email="admin@example.com", password="secret"
        )
        SocialAccount.objects.create(  # ty: ignore[unresolved-attribute]
            user=user, provider="discord", uid="123"
        )
        approved = ApprovedDiscordUser.objects.create(discord_id="123")
        self.client.force_login(user, backend="config.auth.DiscordOnlyBackend")
        self.assertEqual(self.client.get("/admin/").status_code, 200)
        self.assertEqual(self.client.get("/").status_code, 200)
        self.assertEqual(self.client.get("/auth/frontend/").status_code, 204)
        approved.enabled = False
        approved.save()
        self.assertEqual(self.client.get("/admin/").status_code, 302)
        self.assertEqual(self.client.get("/").status_code, 302)
        self.assertEqual(self.client.get("/auth/frontend/").status_code, 302)
