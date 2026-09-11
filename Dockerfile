# Stage 1: Build
FROM mcr.microsoft.com/dotnet/sdk:8.0-alpine AS build
WORKDIR /src
COPY QobuzDLX.Web.csproj .
RUN dotnet restore
COPY . .
RUN dotnet publish -c Release -o /app --no-restore

# Stage 2: Runtime
FROM mcr.microsoft.com/dotnet/aspnet:8.0-alpine AS runtime
WORKDIR /app

# flac : correction du MD5 des fichiers FLAC
# icu-libs + icu-data-full : l'image Alpine tourne en globalization-invariant mode par défaut,
#   sans quoi aucune culture nommée (fr-FR, en-US) n'existe — formats de nombres et tri
#   alphabétique tenant compte des accents.
RUN apk add --no-cache flac icu-libs icu-data-full

# Create non-root user
RUN addgroup -g 1000 qbdlx && adduser -u 1000 -G qbdlx -D qbdlx

# Create volumes and set ownership
RUN mkdir -p /downloads /config && chown -R qbdlx:qbdlx /downloads /config

COPY --from=build --chown=qbdlx:qbdlx /app .

ENV ASPNETCORE_URLS=http://+:8080
ENV CONFIG_PATH=/config
# L'image de base positionne cette variable à true ; ICU étant installé, on la désactive.
ENV DOTNET_SYSTEM_GLOBALIZATION_INVARIANT=false
EXPOSE 8080

# /healthz answers without the access password. busybox wget ships with the Alpine base image.
HEALTHCHECK --interval=30s --timeout=5s --start-period=20s --retries=3 \
  CMD wget -q -O /dev/null http://127.0.0.1:8080/healthz || exit 1

VOLUME ["/downloads", "/config"]

USER qbdlx
ENTRYPOINT ["dotnet", "QobuzDLX.Web.dll"]
