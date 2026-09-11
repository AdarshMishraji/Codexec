FROM mcr.microsoft.com/dotnet/sdk:10.0-alpine AS builder

FROM alpine:3.24.1

RUN apk add --no-cache libstdc++ libgcc icu-libs icu-data-full

COPY --from=builder /usr/share/dotnet/shared/Microsoft.NETCore.App /usr/share/dotnet/shared/Microsoft.NETCore.App
COPY --from=builder /usr/share/dotnet/host /usr/share/dotnet/host
COPY --from=builder /usr/share/dotnet/dotnet /usr/share/dotnet/dotnet
COPY --from=builder /usr/share/dotnet/sdk/*/Roslyn/bincore /usr/share/dotnet/sdk/10.0.401/Roslyn/bincore
COPY --from=builder /usr/share/dotnet/packs/Microsoft.NETCore.App.Ref /usr/share/dotnet/packs/Microsoft.NETCore.App.Ref
COPY --from=builder /usr/share/dotnet/LICENSE.txt /usr/share/dotnet/ThirdPartyNotices.txt /usr/share/dotnet/

RUN mkdir -p /opt/codexec && cat > /opt/codexec/main.runtimeconfig.json <<'EOF'
{
  "runtimeOptions": {
    "tfm": "net10.0",
    "framework": { "name": "Microsoft.NETCore.App", "version": "10.0.0" }
  }
}
EOF

RUN ln -s /usr/share/dotnet/dotnet /usr/bin/dotnet
