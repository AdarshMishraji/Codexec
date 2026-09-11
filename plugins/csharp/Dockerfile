# codexec C# plugin image
#
# LeetCode target: C# 14 with the .NET 10 runtime. No extra NuGet packages
# are documented for this language (unlike Go/Swift/JS elsewhere in this
# repo), so unlike those plugins there's no throwaway template project to
# pre-restore dependencies into - the SDK image already bundles everything
# a submission needs: the Roslyn compiler and the full BCL reference
# assemblies.
#
# Submissions are compiled with `csc` (the Roslyn compiler) directly, NOT
# `dotnet build`/`dotnet run` - this is load-bearing, not a style choice.
# `dotnet build` (and `run`/`new`/`restore`) all go through the full SDK
# CLI (`Microsoft.DotNet.Cli.Program.Main`), which on every invocation
# calls `ConfigureDotNetForFirstTimeUse` -> NuGet's `MigrationRunner`,
# which unconditionally creates a named `Mutex` backed by a file under
# `/tmp/.dotnet/shm/session1` - a path .NET's named-synchronization-object
# implementation hardcodes on Linux, ignoring `TMPDIR` entirely. Since the
# sandbox's rootfs is read-only with no `/tmp` tmpfs, every single
# `dotnet build`/`run` invocation fails outright with "Read-only file
# system" before it even gets to compiling anything - confirmed live
# against this exact image under `docker run --read-only` + PATH-only env
# (matching spec.rs). `dotnet exec <csc.dll>` (compiling) and plain
# `dotnet <already-built.dll>` (running) both go through a much thinner
# apphost-launch path that never touches this first-time-use logic at
# all - verified end-to-end with no HOME/TMPDIR overrides needed anywhere,
# unlike every other compiled-language plugin in this repo.
#
# The full SDK image is ~1GB, but bypassing `dotnet build` entirely means
# almost none of it is actually needed at submission time: measured, the
# ~1GB breaks down into ~425MB of "sdk" (of which DotnetTools, the MSBuild
# Sdks resolver files, and the entire F# compiler together are ~230MB -
# none reachable from `csc`/`dotnet exec`), ~103MB of "packs" (half of
# which is the ASP.NET Core reference pack, irrelevant to a plain console
# submission), and ~113MB of "shared" runtimes (a third of which is the
# ASP.NET Core shared runtime). What's actually load-bearing for
# `dotnet exec <csc.dll> ...` (compiling) and `dotnet <dll>` (running) is
# just four things, confirmed self-contained by inspection (Roslyn/bincore
# ships its own private copies of Microsoft.CodeAnalysis*.dll rather than
# depending on the SDK-root copies) and then verified end-to-end:
#   - shared/Microsoft.NETCore.App/<ver> - the actual CLR + BCL runtime.
#   - host/ - libhostfxr, which locates and loads that runtime.
#   - sdk/<ver>/Roslyn/bincore - just the C# compiler, not the rest of the
#     SDK (MSBuild engine/tasks, NuGet client, F#, VB, dev tools, most
#     locale resource dirs, docs).
#   - packs/Microsoft.NETCore.App.Ref/<ver> - the reference assemblies
#     compile_cmd passes to csc via -r:.
# As with Dart and Rust elsewhere in this repo, actually shrinking the
# image this way requires a genuinely separate final stage on a fresh
# base - not a later RUN/rm layer on top of `FROM ...-alpine`, which would
# just hide the untrimmed files behind a whiteout while the base image's
# own layers (still fetched and stored) keep the full ~1GB.
FROM mcr.microsoft.com/dotnet/sdk:10.0-alpine AS builder

# alpine:3.24.1 is this exact SDK image's own base (confirmed via
# /etc/os-release) - matching it exactly means the musl ABI the CLR/csc's
# native host binaries were built against is guaranteed compatible.
FROM alpine:3.24.1

# The CLR's native host (libhostfxr.so) and runtime (libcoreclr.so) link
# against libgcc_s/libstdc++ (confirmed via `ldd`), neither on a bare
# Alpine base by default. icu-libs + icu-data-full give the runtime real
# culture-aware string/globalization behavior (casing, comparison,
# formatting) matching a normal .NET install - deliberately NOT running in
# "invariant globalization" mode (which would need no ICU at all, saving
# ~36MB) since that measurably changes string behavior for some inputs,
# and matching real .NET semantics matters more here than the extra size.
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

# The muxer resolves its own install root from its real path, so a plain
# symlink onto the sandbox's hardcoded PATH (see spec.rs) works the same
# way it does for the other plugins here that symlink a base-image binary
# into place (e.g. Go, Rust) - verified end-to-end below.
RUN ln -s /usr/share/dotnet/dotnet /usr/bin/dotnet
