use std::{
    collections::HashSet,
    env::{self, set_current_dir},
    ffi::OsStr,
    path::PathBuf,
    process::{self, exit},
};

use anyhow::Result;
use clap::{Parser, Subcommand};
use fs_err as fs;

#[derive(Debug, Parser)]
struct Args {
    #[clap(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    #[clap(disable_help_flag = true, disable_help_subcommand = true)]
    GenDotnet {
        #[clap(long, short)]
        target: Vec<String>,
        #[clap(long, short)]
        release: bool,
        #[arg(
            trailing_var_arg = true,
            allow_hyphen_values = true,
            value_name = "BUILD-OPTIONS"
        )]
        args: Vec<String>,
    },
}

fn main() -> Result<()> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    set_current_dir(root).unwrap();

    let args = Args::parse();
    match args.command {
        Command::GenDotnet {
            target,
            mut args,
            release,
        } => {
            args.extend([
                "--out-dir=dotnet/Core".into(),
                "--config=uniffi.toml".into(),
                "packages/ffi/src/CamreteCore.udl".into(),
            ]);
            launch_bin("gen-dotnet", &args)?;

            let default_triple = default_triple();

            eprintln!("--- Building dynamic libs ---");
            let mut target: HashSet<String> = HashSet::from_iter(target);
            target.insert(default_triple.clone());

            for triple in target {
                eprintln!("Building {triple}");
                let platform = lookup_tuple(&triple);

                let mut cmd = cargo();
                cmd.args(["build", "-p", "camrete-ffi", "--target", &triple]);
                if release {
                    cmd.arg("--release");
                }

                let success = cmd.status()?.success();
                if !success {
                    exit(1);
                }

                let dll_name = format!("{}camrete{}", platform.dll_prefix, platform.dll_suffix);

                let rt_dir = PathBuf::from("dotnet/Core/runtimes")
                    .join(platform.rid)
                    .join("native");

                fs::create_dir_all(&rt_dir)?;

                let dll_path = PathBuf::from("target")
                    .join(&triple)
                    .join(if release { "release" } else { "debug" })
                    .join(&dll_name);

                fs::copy(&dll_path, rt_dir.join(&dll_name))?;

                // Dotnet's platform-aware assembly resolution doesn't work for ProjectReferences.
                // As a work around, copy the native dylib into a separate location where it can be easily
                // consumed.
                if triple == default_triple {
                    let native_dir = PathBuf::from("dotnet/Core/runtimes/native");
                    fs::create_dir_all(&native_dir)?;
                    fs::copy(dll_path, native_dir.join(&dll_name))?;
                }
            }
        }
    }

    Ok(())
}

fn launch_bin(name: &str, args: impl IntoIterator<Item = impl AsRef<OsStr>>) -> Result<()> {
    let mut cmd = cargo();
    cmd.args(["run", "-p", "xtask", "--bin", name, "--"]);
    cmd.args(args);

    let success = cmd.status()?.success();
    if !success {
        exit(1);
    }

    Ok(())
}

fn cargo() -> process::Command {
    let cargo = env::var("CARGO").unwrap();
    process::Command::new(cargo)
}

struct TripleDetails {
    rid: &'static str,
    dll_prefix: &'static str,
    dll_suffix: &'static str,
}

fn lookup_tuple(target: &str) -> TripleDetails {
    let parts = target.split("-").collect::<Vec<_>>();
    let rid = match *parts.as_slice() {
        ["aarch64", "apple", "darwin"] => "osx-arm64",
        ["x86_64", "apple", "darwin"] => "osx-x64",

        ["aarch64", "pc", "windows", _] => "win-arm64",
        ["x86_64", "pc", "windows", _] => "win-x64",

        ["x86_64", "unknown", "linux", "gnu"] => "linux-x64",
        ["x86_64", "unknown", "linux", "musl"] => "linux-musl-x64",
        ["aarch64", "unknown", "linux", "gnu"] => "linux-arm64",
        ["aarch64", "unknown", "linux", "musl"] => "linux-musl-arm64",
        ["armv7", "unknown", "linux", "gnueabihf"] => "linux-arm",

        _ => panic!("no dotnet RID not known for {target:?}"),
    };

    let (dll_prefix, dll_suffix) = match parts[2] {
        "darwin" => ("lib", ".dylib"),
        "linux" => ("lib", ".so"),
        "windows" => ("", ".dll"),
        _ => unreachable!(),
    };

    TripleDetails { rid, dll_prefix, dll_suffix }
}

fn default_triple() -> String {
    let mut rustc = process::Command::new("rustc");
    rustc.args(["--print", "host-tuple"]);
    let out = rustc.output().unwrap();
    let triple = String::from_utf8(out.stdout).unwrap();
    triple.trim().to_string()
}
