use std::{
    collections::HashSet,
    env::{self, set_current_dir},
    ffi::{OsStr, OsString},
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
        args: Vec<OsString>,
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
            let default_platform = default_triple();

            eprintln!("--- Building dynamic libs & .NET bindings ---");
            let mut target: HashSet<String> = HashSet::from_iter(target);
            target.insert(default_platform.triple.clone());

            for triple in target {
                eprintln!("Building {triple}");
                let platform = lookup_triple(triple);

                let mut cmd = cargo();
                cmd.args(["build", "-p", "camrete-ffi", "--target", &platform.triple]);
                if release {
                    cmd.arg("--release");
                }

                let success = cmd.status()?.success();
                if !success {
                    exit(1);
                }

                // Copy DLL to respective platform directory.

                let rt_dir = PathBuf::from("dotnet/Core/runtimes")
                    .join(platform.rid)
                    .join("native");

                fs::create_dir_all(&rt_dir)?;

                let dll_name = platform.dll("camrete");
                let dll_path = platform.target_dir(release).join(&dll_name);

                fs::copy(&dll_path, rt_dir.join(&dll_name))?;

                if platform.triple == default_platform.triple {
                    // Dotnet's platform-aware assembly resolution doesn't work for ProjectReferences.
                    // As a work around, ALSO copy the native dylib into a separate location where it
                    // can be easily consumed.
                    let native_dir = PathBuf::from("dotnet/Core/runtimes/native");
                    fs::create_dir_all(&native_dir)?;
                    fs::copy(dll_path, native_dir.join(&dll_name))?;

                    // Generate .NET bindings via DLL metadata
                    eprintln!("Generating .NET source code");

                    args.extend([
                        "--out-dir=dotnet/Core".into(),
                        "--config=uniffi.toml".into(),
                        "--library".into(),
                        default_platform
                            .target_dir(release)
                            .join(default_platform.dll("camrete"))
                            .into_os_string(),
                    ]);
                    launch_bin("gen-dotnet", &args)?;
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

fn lookup_triple(triple: String) -> TripleDetails {
    let parts = triple.split("-").collect::<Vec<_>>();
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

        _ => panic!("no dotnet RID not known for {triple:?}"),
    };

    let (dll_prefix, dll_suffix) = match parts[2] {
        "darwin" => ("lib", ".dylib"),
        "linux" => ("lib", ".so"),
        "windows" => ("", ".dll"),
        _ => unreachable!(),
    };

    TripleDetails {
        triple,
        rid,
        dll_prefix,
        dll_suffix,
    }
}

fn default_triple() -> TripleDetails {
    let mut rustc = process::Command::new("rustc");
    rustc.args(["--print", "host-tuple"]);
    let out = rustc.output().unwrap();
    let triple = String::from_utf8(out.stdout).unwrap();
    lookup_triple(triple.trim().to_string())
}

#[derive(Debug, Clone)]
struct TripleDetails {
    triple: String,
    rid: &'static str,
    dll_prefix: &'static str,
    dll_suffix: &'static str,
}

impl TripleDetails {
    fn dll(&self, base: &str) -> String {
        format!("{}{base}{}", self.dll_prefix, self.dll_suffix)
    }

    fn target_dir(&self, release: bool) -> PathBuf {
        PathBuf::from("target")
            .join(&self.triple)
            .join(if release { "release" } else { "debug" })
    }
}
