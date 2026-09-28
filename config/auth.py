from allauth.socialaccount.models import SocialAccount
from django.contrib.auth.backends import ModelBackend

from polls.models import ApprovedDiscordUser


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
