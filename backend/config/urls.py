"""
URL configuration for pollingjuegos project.

The `urlpatterns` list routes URLs to views. For more information please see:
    https://docs.djangoproject.com/en/6.1/topics/http/urls/
Examples:
Function views
    1. Add an import:  from my_app import views
    2. Add a URL to urlpatterns:  path('', views.home, name='home')
Class-based views
    1. Add an import:  from other_app.views import Home
    2. Add a URL to urlpatterns:  path('', Home.as_view(), name='home')
Including another URLconf
    1. Import the include() function: from django.urls import include, path
    2. Add a URL to urlpatterns:  path('blog/', include('blog.urls'))
"""

from django.conf import settings
from django.contrib import admin
from django.contrib.auth.decorators import login_required
from django.contrib.auth.views import redirect_to_login
from django.http import HttpResponse
from django.urls import include, path

from polls.api import api


@login_required
def home(_request):
    return HttpResponse(b"Signed in via Discord.")


def frontend_access(request):
    """Caddy checks the session before serving any frontend file."""
    if not request.user.is_authenticated:
        return redirect_to_login("/", login_url=settings.LOGIN_URL)
    return HttpResponse(status=204)


urlpatterns = [
    path("", home, name="home"),
    path("auth/frontend/", frontend_access, name="frontend_access"),
    path("admin/", admin.site.urls),
    path("api/", api.urls),
    path("accounts/", include("allauth.urls")),
]
