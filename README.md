# Camrete

A subset of CKAN, the Kerbal Space Program mod manager, which uses a relational database to improve speed.

## Features

Camrete is only a re-implementation of CKAN's repository data model, so it doesn't know how to work with game instances or anything. Currently you can use the `camrete update` command to download CKAN's online mod repository, and `camrete show` to view info about a mod.

Project goals:

- Create a C# package for querying CKAN repositories.
- Use a relational database to create a faster implementation of a subset of CKAN Core.
  - As it stands, the `update` command is a similar speed to CKAN's implementation.
  - Currently, the `camrete show` command seems to be over 100x faster than `ckan show`.
- (Maybe) Look into creating packages for other languages, like Swift, to make it easier to create a GUI for CKAN.

## Setup

You need [`rustup`](https://rustup.rs) installed to build Camrete.

Run this command to install Camrete's sample command-line app:

```shell
cargo install --path packages/cli

# Use it:
camrete show ROSolar
```

Run these commands to build the version of the command-line app written in .NET:

```shell
cd dotnet/CLI
cargo xtask create-bindings
dotnet publish

# Use it:
./bin/Release/net8.0/publish/Camrete.CLI show ROSolar
```

## .NET bindings

Camrete itself is a Rust project, but it has bindings to C#. The package containing the bindings, as well as a sample application written in C#, are located in the `dotnet` directory.

Currently, the .NET bindings are automatically generated from the code in `packages/ffi`. The generated code isn't checked into Git, so you have to run this command to generate it:

```shell
cargo xtask create-bindings
```

This will populate the `Camrete.Core` package as well as generate the DLLs it needs to run.

## Cross compiling

Since Camrete compiles to native code, it needs a separate build for each platform. You can cross-compile it to several other platforms, which is especially desirable for building a multi-platform .NET package.

To cross compile Camrete to a different OS, you need to install [`cross`](https://github.com/cross-rs/cross) and Docker.

When using Cross, you can build it just like normal, except replacing the command name:

```shell
cross build -p camrete-ffi --target x86_64-unknown-linux-gnu
```

Alternatively, you can use xtask for this, which might be more useful because it also copies the cross-compiled  DLLs into the .NET package (xtask will still do everything it normally does, like generating bindings). You can specify one or more multiple extra build targets by specifying the `-t` flag:

```shell
cargo xtask create-bindings -t x86_64-unknown-linux-gnu -t aarch64-unknown-linux-gnu
```

Here are the targets that work best with cross-compiling from any platform:

- `x86_64-unknown-linux-gnu` (x64 Linux)
- `aarch64-unknown-linux-gnu` (arm64 Linux)
- `x86_64-pc-windows-gnu` (x64 Windows, no MSVC)

These targets can be compiled to from the a computer running the same OS, for licensing reasons:

- `aarch64-pc-windows-msvc` (arm64 Windows)
- `aarch64-apple-darwin` (arm64 macOS)
- `x86_64-apple-darwin` (x86_64 macOS)

## Testing

Run unit tests using `cargo test`.

Run benchmarks with `cargo bench --bench <bench_name>`.
