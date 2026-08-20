use std::collections::HashSet;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use globset::{Glob, GlobBuilder, GlobSet, GlobSetBuilder};

use crate::cli::Cli;
use crate::config::{Config, PathOverride};

const BUILTIN_DIR_EXCLUDES: &[&str] = &[
    ".git",
    ".hg",
    ".svn",
    "node_modules",
    "vendor",
    "vendors",
    "third_party",
    "third-party",
    "deps",
    "Pods",
    ".venv",
    "venv",
    ".eggs",
    "site-packages",
    "bower_components",
    "jspm_packages",
    "Carthage",
    "generated",
    "gen",
    "generated-src",
    "generated_sources",
    "autogen",
    "target",
    "build",
    "dist",
    "out",
    "obj",
    "coverage",
    "htmlcov",
    ".next",
    ".nuxt",
    ".svelte-kit",
    ".output",
    ".gradle",
    ".build",
    "DerivedData",
    ".dart_tool",
    ".terraform",
    ".terragrunt-cache",
    ".vercel",
    ".serverless",
    ".turbo",
    ".cache",
    ".parcel-cache",
    ".vite",
    ".angular",
    ".pytest_cache",
    ".mypy_cache",
    ".ruff_cache",
    ".tox",
    ".nox",
    "cdk.out",
    ".aws-sam",
    "__pycache__",
];

#[derive(Debug, Default, Clone)]
pub struct Scope {
    pub files: HashSet<PathBuf>,
    pub dirs: Vec<PathBuf>,
}

impl Scope {
    pub fn is_explicit_file(&self, path: &Path) -> bool {
        self.files.contains(path)
    }

    pub fn matching_dir<'a>(&'a self, path: &Path) -> Option<&'a PathBuf> {
        self.dirs
            .iter()
            .filter(|dir| path.starts_with(dir))
            .max_by_key(|dir| dir.components().count())
    }

    pub fn contains(&self, path: &Path) -> bool {
        self.is_explicit_file(path) || self.matching_dir(path).is_some()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Limits {
    pub limit: usize,
    pub warn_percent: u8,
}

impl Limits {
    pub fn warn_at(self) -> usize {
        let value = (self.limit as u128 * self.warn_percent as u128).div_ceil(100);
        usize::try_from(value).unwrap_or(usize::MAX)
    }
}

pub struct Policy {
    root: PathBuf,
    excludes: GlobSet,
    includes: GlobSet,
    only: Option<GlobSet>,
    exempt: HashSet<String>,
    default_types: bool,
    default_excludes: bool,
    overrides: Vec<CompiledOverride>,
    global: Limits,
    cli_limit: Option<usize>,
    cli_warn_percent: Option<u8>,
}

struct CompiledOverride {
    files: GlobSet,
    limit: Option<usize>,
    warn_percent: Option<u8>,
}

impl Policy {
    pub fn new(config: &Config, cli: &Cli, root: &Path) -> Result<Self> {
        let excludes = compile_globs(config.exclude.iter().chain(&cli.exclude), "exclude")?;
        let includes = compile_globs(config.include.iter().chain(&cli.include), "include")?;
        let only = (!cli.only.is_empty())
            .then(|| compile_globs(cli.only.iter(), "only"))
            .transpose()?;
        let overrides = config
            .overrides
            .iter()
            .map(compile_override)
            .collect::<Result<Vec<_>>>()?;

        Ok(Self {
            root: root.to_path_buf(),
            excludes,
            includes,
            only,
            exempt: config.exempt_files.iter().cloned().collect(),
            default_types: config.default_types,
            default_excludes: config.default_excludes(cli),
            overrides,
            global: Limits {
                limit: config.limit,
                warn_percent: config.warn_percent,
            },
            cli_limit: cli.limit,
            cli_warn_percent: cli.warn_percent,
        })
    }

    pub fn should_scan(&self, path: &Path, relative: &str, scope: &Scope, no_exempt: bool) -> bool {
        if self.excludes.is_match(relative) {
            return false;
        }
        if !no_exempt && self.exempt.contains(relative) {
            return false;
        }

        let explicit_file = scope.is_explicit_file(path);
        if explicit_file {
            return true;
        }

        let include_match = if let Some(only) = &self.only {
            only.is_match(relative)
        } else {
            self.includes.is_match(relative)
        };

        let source_match = if self.only.is_some() {
            include_match
        } else {
            include_match || (self.default_types && is_source_path(path))
        };
        if !source_match {
            return false;
        }

        if !self.default_excludes || include_match {
            return true;
        }

        if is_generated_filename(path) {
            return false;
        }

        let builtin_check_path = scope
            .matching_dir(path)
            .and_then(|dir| path.strip_prefix(dir).ok())
            .unwrap_or_else(|| path.strip_prefix(&self.root).unwrap_or(path));
        !has_builtin_excluded_component(builtin_check_path)
    }

    pub fn limits_for(&self, relative: &str) -> Limits {
        let mut limits = self.global;
        for rule in &self.overrides {
            if rule.files.is_match(relative) {
                if let Some(limit) = rule.limit {
                    limits.limit = limit;
                }
                if let Some(warn_percent) = rule.warn_percent {
                    limits.warn_percent = warn_percent;
                }
            }
        }
        if let Some(limit) = self.cli_limit {
            limits.limit = limit;
        }
        if let Some(warn_percent) = self.cli_warn_percent {
            limits.warn_percent = warn_percent;
        }
        limits
    }

    pub fn can_prune_builtin_dirs(&self) -> bool {
        self.default_excludes && self.only.is_none() && self.includes.is_empty()
    }
}

fn compile_override(rule: &PathOverride) -> Result<CompiledOverride> {
    Ok(CompiledOverride {
        files: compile_globs(rule.files.iter(), "override")?,
        limit: rule.limit,
        warn_percent: rule.warn_percent,
    })
}

fn compile_globs<'a>(patterns: impl Iterator<Item = &'a String>, kind: &str) -> Result<GlobSet> {
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        let glob =
            build_glob(pattern).with_context(|| format!("invalid {kind} glob '{pattern}'"))?;
        builder.add(glob);
    }
    builder
        .build()
        .with_context(|| format!("failed to compile {kind} globs"))
}

fn build_glob(pattern: &str) -> Result<Glob> {
    GlobBuilder::new(&pattern.replace('\\', "/"))
        .literal_separator(true)
        .backslash_escape(false)
        .build()
        .map_err(Into::into)
}

pub fn has_builtin_excluded_component(path: &Path) -> bool {
    path.components().any(|component| {
        let value = component.as_os_str().to_string_lossy();
        BUILTIN_DIR_EXCLUDES
            .iter()
            .any(|excluded| value == *excluded)
    })
}

fn is_generated_filename(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    let lower = name.to_ascii_lowercase();
    lower.contains(".generated.")
        || lower.contains(".gen.")
        || lower.starts_with("zz_generated.")
        || lower.ends_with(".pb.go")
        || lower.ends_with(".pb.cc")
        || lower.ends_with(".pb.h")
        || lower.ends_with("_pb2.py")
        || lower.ends_with("_pb2_grpc.py")
        || lower.ends_with(".g.dart")
        || lower.ends_with(".freezed.dart")
        || lower.ends_with(".designer.cs")
}

fn is_source_path(path: &Path) -> bool {
    let Some(extension) = path.extension().and_then(|value| value.to_str()) else {
        return false;
    };
    let extension = extension.to_ascii_lowercase();
    matches!(
        extension.as_str(),
        "rs" | "go"
            | "js"
            | "jsx"
            | "mjs"
            | "cjs"
            | "ts"
            | "tsx"
            | "mts"
            | "cts"
            | "py"
            | "pyi"
            | "pyx"
            | "pxd"
            | "swift"
            | "m"
            | "mm"
            | "c"
            | "h"
            | "cc"
            | "cpp"
            | "cxx"
            | "hh"
            | "hpp"
            | "hxx"
            | "ixx"
            | "java"
            | "kt"
            | "kts"
            | "scala"
            | "sc"
            | "r"
            | "jl"
            | "coffee"
            | "litcoffee"
            | "cs"
            | "cshtml"
            | "razor"
            | "fs"
            | "fsi"
            | "fsx"
            | "vb"
            | "rb"
            | "erb"
            | "php"
            | "php3"
            | "php4"
            | "php5"
            | "phtml"
            | "lua"
            | "sh"
            | "bash"
            | "zsh"
            | "fish"
            | "ps1"
            | "psm1"
            | "bat"
            | "cmd"
            | "ex"
            | "exs"
            | "erl"
            | "hrl"
            | "hs"
            | "lhs"
            | "ml"
            | "mli"
            | "re"
            | "rei"
            | "zig"
            | "nim"
            | "cr"
            | "d"
            | "dart"
            | "gd"
            | "gdshader"
            | "qml"
            | "qbs"
            | "hx"
            | "res"
            | "resi"
            | "purs"
            | "roc"
            | "pony"
            | "mo"
            | "moon"
            | "vue"
            | "svelte"
            | "astro"
            | "html"
            | "htm"
            | "css"
            | "scss"
            | "sass"
            | "less"
            | "styl"
            | "pcss"
            | "sql"
            | "psql"
            | "plsql"
            | "proto"
            | "capnp"
            | "thrift"
            | "avdl"
            | "smithy"
            | "wit"
            | "graphql"
            | "gql"
            | "tf"
            | "hcl"
            | "nix"
            | "cue"
            | "sol"
            | "move"
            | "cairo"
            | "circom"
            | "sway"
            | "vy"
            | "teal"
            | "clj"
            | "cljs"
            | "cljc"
            | "groovy"
            | "gvy"
            | "gsh"
            | "rkt"
            | "scm"
            | "ss"
            | "lisp"
            | "cl"
            | "el"
            | "vim"
            | "awk"
            | "tcl"
            | "lean"
            | "thy"
            | "agda"
            | "lagda"
            | "idr"
            | "idr2"
            | "elm"
            | "gleam"
            | "odin"
            | "vala"
            | "vapi"
            | "sml"
            | "sig"
            | "fun"
            | "pas"
            | "pp"
            | "f"
            | "f77"
            | "f90"
            | "f95"
            | "f03"
            | "f08"
            | "for"
            | "fpp"
            | "cob"
            | "cbl"
            | "adb"
            | "ads"
            | "pl"
            | "pm"
            | "raku"
            | "sas"
            | "ejs"
            | "hbs"
            | "handlebars"
            | "mustache"
            | "pug"
            | "haml"
            | "slim"
            | "twig"
            | "jinja"
            | "jinja2"
            | "liquid"
            | "njk"
            | "wgsl"
            | "glsl"
            | "vert"
            | "frag"
            | "hlsl"
            | "shader"
            | "compute"
            | "metal"
            | "cu"
            | "cuh"
            | "wat"
            | "wast"
            | "rego"
            | "dhall"
            | "jsonnet"
            | "libsonnet"
            | "prisma"
            | "pkl"
            | "bzl"
            | "star"
            | "cmake"
            | "xsl"
            | "xslt"
            | "xq"
            | "xquery"
            | "v"
            | "sv"
            | "svh"
            | "vhd"
            | "vhdl"
            | "asm"
            | "inc"
            | "s"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_source_extensions_but_not_special_names() {
        assert!(is_source_path(Path::new("src/main.rs")));
        assert!(is_source_path(Path::new("web/App.TSX")));
        assert!(is_source_path(Path::new("schema.proto")));
        assert!(!is_source_path(Path::new("Makefile")));
        assert!(!is_source_path(Path::new("Cargo.lock")));
        assert!(!is_source_path(Path::new("fixture.json")));
    }

    #[test]
    fn recognizes_generated_filenames_conservatively() {
        assert!(is_generated_filename(Path::new("api.generated.ts")));
        assert!(is_generated_filename(Path::new("thing.pb.go")));
        assert!(is_generated_filename(Path::new("form.Designer.cs")));
        assert!(!is_generated_filename(Path::new("generated_by_hand.rs")));
    }
}
