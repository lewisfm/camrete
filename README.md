# Camrete

A subset of CKAN, the Kerbal Space Program mod manager, which uses a relational database to improve speed.

## Setup

You need [`rustup`](https://rustup.rs) installed to build Camrete.

Run this command to install its command-line tool:

```terminal
cargo install --path packages/cli
```

## .NET bindings

Camrete itself is a Rust project, but it has bindings to C#. The package containing the bindings, as well as a sample application written in C#, are located in the `dotnet` directory.

Currently, the .NET bindings are automatically generated from the code in `packages/ffi`. They aren't checked into Git, so you have to run this command to generate them:

```terminal
cargo xtask gen-dotnet
```

This will populate the `Camrete.Core` package as well as generate the DLLs it needs to run.

## Cross compiling

Since Camrete compiles to native code, it needs a separate build for each platform. You can cross-compile it to several other platforms, which is especially desirable for building a multi-platform .NET package.

To cross compile Camrete to a different OS, you need to install [`cross`](https://github.com/cross-rs/cross) and Docker.

When using Cross, you can build it just like normal, except replacing the command name:

```terminal
cross build -p camrete-ffi --target x86_64-unknown-linux-gnu
```

Alternatively, you can use xtask for this, which might be more useful because it also copies the cross-compiled  DLLs into the .NET package (xtask will still do everything it normally does, like generating bindings). You can specify one or more multiple extra build targets by specifying the `-t` flag:

```terminal
cargo xtask gen-dotnet -t x86_64-unknown-linux-gnu -t aarch64-unknown-linux-gnu
```

Here are the targets that work best with cross-compiling from any platform:

- `x86_64-unknown-linux-gnu` (x64 Linux)
- `aarch64-unknown-linux-gnu` (arm64 Linux)
- `x86_64-pc-windows-gnu` (x64 Windows, no MSVC)

These targets can be compiled to from the a computer running the same OS, for licensing reasons:

- `aarch64-pc-windows-msvc` (arm64 Windows)
- `aarch64-apple-darwin` (arm64 macOS)
- `x86_64-apple-darwin` (x86_64 macOS)
