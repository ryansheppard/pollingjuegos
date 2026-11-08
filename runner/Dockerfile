FROM docker.io/library/debian:trixie-slim@sha256:7378260c0a098d27ac206225db5e7d92b8c7c84ea7afccc86eeb513756bbd392

COPY --from=ghcr.io/jdx/mise:2026.10.4-debian /usr/local/bin/mise /usr/local/bin/mise

RUN apt-get update \
  && apt-get install -y --no-install-recommends ca-certificates gh git zip \
  && rm -rf /var/lib/apt/lists/*

RUN useradd --create-home runner && install -d -o runner -g runner /src
USER runner
WORKDIR /src

ENV MISE_TRUSTED_CONFIG_PATHS=/src/pollingjuegos
ENV PATH=/home/runner/.local/share/mise/shims:$PATH

CMD ["sleep", "infinity"]
