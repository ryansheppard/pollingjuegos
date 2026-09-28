from allauth.socialaccount.models import SocialAccount
from django.contrib.auth.backends import ModelBackend
from django.http import JsonResponse

from polls.models import ApprovedDiscordUser


class ApiLoginRequiredMiddleware:
    """Guard every API path, including Ninja's debug schema and docs views."""

    def __init__(self, get_response):
        self.get_response = get_response

    def __call__(self, request):
        if request.path.startswith("/api/") and not request.user.is_authenticated:
            return JsonResponse({"detail": "Unauthorized"}, status=401)
        return self.get_response(request)


class DiscordOnlyBackend(ModelBackend):
    """OAuth creates sessions; password credentials never do."""

    def authenticate(self, request, username=None, password=None, **kwargs):
        return None

    def get_user(self, user_id):
        user = super().get_user(user_id)
        if user is None:
            return None
        approved_ids = ApprovedDiscordUser.objects.filter(enabled=True).values(
            "discord_id"
        )
        if not SocialAccount.objects.filter(  # ty: ignore[unresolved-attribute]
            user=user, provider="discord", uid__in=approved_ids
        ).exists():
            return None
        return user
