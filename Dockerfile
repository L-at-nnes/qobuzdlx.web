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

# Create volumes
RUN mkdir -p /downloads /config

COPY --from=build /app .

ENV ASPNETCORE_URLS=http://+:8080
ENV CONFIG_PATH=/config
EXPOSE 8080

VOLUME ["/downloads", "/config"]

ENTRYPOINT ["dotnet", "QobuzDLX.Web.dll"]
