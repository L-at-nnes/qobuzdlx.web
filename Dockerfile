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

# Install flac for MD5 fix support
RUN apk add --no-cache flac

# Create non-root user
RUN addgroup -g 1000 qbdlx && adduser -u 1000 -G qbdlx -D qbdlx

# Create volumes and set ownership
RUN mkdir -p /downloads /config && chown -R qbdlx:qbdlx /downloads /config

COPY --from=build --chown=qbdlx:qbdlx /app .

ENV ASPNETCORE_URLS=http://+:8080
ENV CONFIG_PATH=/config
EXPOSE 8080

VOLUME ["/downloads", "/config"]

USER qbdlx
ENTRYPOINT ["dotnet", "QobuzDLX.Web.dll"]
