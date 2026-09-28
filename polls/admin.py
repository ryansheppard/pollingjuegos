from django.contrib import admin

from .models import ApprovedDiscordUser, Ballot, PollWeek, Quarterback, Team, Vote


@admin.register(ApprovedDiscordUser)
class ApprovedDiscordUserAdmin(admin.ModelAdmin):
    list_display = ("discord_id", "enabled", "note")
    list_filter = ("enabled",)
    search_fields = ("discord_id", "note")


@admin.register(Team)
class TeamAdmin(admin.ModelAdmin):
    list_display = ("name", "abbreviation")
    search_fields = ("name", "abbreviation")


@admin.register(Quarterback)
class QuarterbackAdmin(admin.ModelAdmin):
    list_display = ("name", "team")
    search_fields = ("name",)
    list_filter = ("team",)


@admin.register(PollWeek)
class PollWeekAdmin(admin.ModelAdmin):
    list_display = ("season", "week", "closes_at", "is_closed")


class VoteInline(admin.TabularInline):
    model = Vote
    extra = 0


@admin.register(Ballot)
class BallotAdmin(admin.ModelAdmin):
    list_display = ("voter", "poll_week")
    list_filter = ("poll_week",)
    inlines = (VoteInline,)
