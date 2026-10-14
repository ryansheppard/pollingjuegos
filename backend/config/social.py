from allauth.core.exceptions import ImmediateHttpResponse
from allauth.socialaccount.adapter import DefaultSocialAccountAdapter
from django.http import HttpResponseForbidden

from polls.models import ApprovedDiscordUser


class DiscordAllowlistAdapter(DefaultSocialAccountAdapter):
    def approved(self, sociallogin):
        return (
            sociallogin.account.provider == "discord"
            and ApprovedDiscordUser.objects.filter(
                discord_id=sociallogin.account.uid, enabled=True
            ).exists()
        )

    def pre_social_login(self, request, sociallogin):
        # Runs for both new and existing accounts, before signup/login/connect.
        if not self.approved(sociallogin):
            raise ImmediateHttpResponse(
                HttpResponseForbidden("Discord account not approved.")
            )

    def is_open_for_signup(self, request, sociallogin):
        # Recheck if a pending social signup is submitted after revocation.
        return self.approved(sociallogin)
