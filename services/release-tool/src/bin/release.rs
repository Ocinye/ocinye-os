//! `ocinye-release`: the publisher of Ocinye update repositories (ADR-0032).
//!
//!   ocinye-release keys init DIR
//!   ocinye-release root create --keys DIR --out FILE [--days N]
//!   ocinye-release channel init --repo DIR --channel C --root FILE
//!   ocinye-release root rotate --repo DIR --channel C --old-keys DIR --new-keys DIR [--days N]
//!   ocinye-release stage --bundle DIR --native TAR --out DIR --channel C
//!         --number N --minimum N --revision N --migrations none|expand|contract:N
//!         --notes-pt T --notes-en T --notes-fr T [--reboot] [--security]
//!         [--synthetic PURPOSE] [--created-at YYYY-MM-DDTHH:MM:SSZ]
//!   ocinye-release publish --repo DIR --channel C --keys DIR --staged DIR
//!   ocinye-release refresh --repo DIR --channel C --keys DIR
//!   ocinye-release verify  --repo DIR --channel C --root FILE
//!   ocinye-release bundle  --repo DIR --channel C --version V --out FILE.ocu
//!
//! Keys stay where the publisher keeps them (0700); nothing here copies a
//! private key anywhere, and the repository it writes holds none. The server
//! that serves the repository needs no key at all.

use ocinye_update_contracts::manifest::{MigrationPolicy, Notes};
use ocinye_update_contracts::{Channel, ReleaseVersion};
use ocinye_update_repository::keys::{create_root, rotate_root, KeySet};
use ocinye_update_repository::publish::{
    init_channel, publish_release, refresh, rotate_keys, Expiry,
};
use ocinye_update_repository::stage::{stage, StageInput};
use ocinye_update_repository::verify::{load_dir, releases};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::ExitCode;

const FLAGS_WITHOUT_VALUE: [&str; 2] = ["--reboot", "--security"];

struct Args {
    values: BTreeMap<String, String>,
    switches: Vec<String>,
}

impl Args {
    fn parse(words: &[String]) -> Result<Self, String> {
        let mut values = BTreeMap::new();
        let mut switches = Vec::new();
        let mut it = words.iter();
        while let Some(w) = it.next() {
            if !w.starts_with("--") {
                return Err(format!("unexpected argument {w}"));
            }
            if FLAGS_WITHOUT_VALUE.contains(&w.as_str()) {
                switches.push(w.clone());
                continue;
            }
            let v = it.next().ok_or_else(|| format!("{w} needs a value"))?;
            if values.insert(w.clone(), v.clone()).is_some() {
                return Err(format!("{w} given twice"));
            }
        }
        Ok(Self { values, switches })
    }
    fn get(&self, k: &str) -> Result<&str, String> {
        self.values
            .get(k)
            .map(String::as_str)
            .ok_or_else(|| format!("{k} is required"))
    }
    fn path(&self, k: &str) -> Result<PathBuf, String> {
        self.get(k).map(PathBuf::from)
    }
    fn channel(&self) -> Result<Channel, String> {
        Channel::parse(self.get("--channel")?)
            .ok_or_else(|| "--channel is development, candidate or stable".into())
    }
    fn number(&self, k: &str) -> Result<u64, String> {
        self.get(k)?.parse().map_err(|_| format!("{k} is a number"))
    }
    fn days(&self) -> Result<jiff::Timestamp, String> {
        let days: i64 = self.values.get("--days").map_or(Ok(365), |d| {
            d.parse().map_err(|_| "--days is a number".to_string())
        })?;
        jiff::Timestamp::now()
            .checked_add(jiff::SignedDuration::from_hours(days * 24))
            .map_err(|e| e.to_string())
    }
    fn only(&self, allowed: &[&str]) -> Result<(), String> {
        for k in self.values.keys().chain(self.switches.iter()) {
            if !allowed.contains(&k.as_str()) {
                return Err(format!("unknown option {k}"));
            }
        }
        Ok(())
    }
}

fn now_utc() -> String {
    let t = jiff::Timestamp::now();
    t.strftime("%Y-%m-%dT%H:%M:%SZ").to_string()
}

async fn run(words: Vec<String>) -> Result<(), String> {
    let e = |x: ocinye_update_repository::Error| x.to_string();
    let w: Vec<&str> = words.iter().map(String::as_str).collect();
    match w.as_slice() {
        ["keys", "init", dir] => {
            KeySet::generate(&PathBuf::from(dir)).map_err(e)?;
            println!("keys written to {dir} (0700); keep them off any server and out of git");
            Ok(())
        }
        ["root", "create", rest @ ..] => {
            let a = Args::parse(&words[2..2 + rest.len()])?;
            a.only(&["--keys", "--out", "--days"])?;
            let keys = KeySet::open(&a.path("--keys")?).map_err(e)?;
            let root = create_root(&keys, 1, a.days()?).await.map_err(e)?;
            std::fs::write(a.path("--out")?, root).map_err(|x| x.to_string())?;
            Ok(())
        }
        ["channel", "init", rest @ ..] => {
            let a = Args::parse(&words[2..2 + rest.len()])?;
            a.only(&["--repo", "--channel", "--root"])?;
            let root = std::fs::read(a.path("--root")?).map_err(|x| x.to_string())?;
            init_channel(&a.path("--repo")?, a.channel()?, &root).map_err(e)
        }
        ["root", "rotate", rest @ ..] => {
            let a = Args::parse(&words[2..2 + rest.len()])?;
            a.only(&["--repo", "--channel", "--old-keys", "--new-keys", "--days"])?;
            let repo = a.path("--repo")?;
            let channel = a.channel()?;
            let old = KeySet::open(&a.path("--old-keys")?).map_err(e)?;
            let new = KeySet::open(&a.path("--new-keys")?).map_err(e)?;
            let current = std::fs::read(repo.join(channel.as_str()).join("metadata/root.json")).map_err(|x| x.to_string())?;
            let next = rotate_root(&current, &old, &new, a.days()?).await.map_err(e)?;
            let v = rotate_keys(&repo, channel, &new, &next, Expiry::for_channel(channel)).await.map_err(e)?;
            println!("root version {v} published; machines follow the chain from their installed root");
            Ok(())
        }
        ["stage", rest @ ..] => {
            let a = Args::parse(&words[1..1 + rest.len()])?;
            a.only(&[
                "--bundle", "--native", "--out", "--channel", "--number", "--minimum", "--revision",
                "--migrations", "--notes-pt", "--notes-en", "--notes-fr", "--reboot", "--security",
                "--synthetic", "--created-at",
            ])?;
            let migrations = match a.get("--migrations")? {
                "none" => MigrationPolicy::None,
                "expand" => MigrationPolicy::Expand,
                other => match other.strip_prefix("contract:").and_then(|n| n.parse().ok()) {
                    Some(requires_source) => MigrationPolicy::Contract { requires_source },
                    None => return Err("--migrations is none, expand or contract:N".into()),
                },
            };
            let input = StageInput {
                bundle_dir: a.path("--bundle")?,
                native_tar: a.path("--native")?,
                out_dir: a.path("--out")?,
                revision: u32::try_from(a.number("--revision")?).map_err(|x| x.to_string())?,
                release_number: a.number("--number")?,
                minimum_source: a.number("--minimum")?,
                channel: a.channel()?,
                created_at: a.values.get("--created-at").cloned().unwrap_or_else(now_utc),
                migrations,
                reboot_required: a.switches.iter().any(|s| s == "--reboot"),
                security: a.switches.iter().any(|s| s == "--security"),
                notes: Notes {
                    pt: a.get("--notes-pt")?.to_owned(),
                    en: a.get("--notes-en")?.to_owned(),
                    fr: a.get("--notes-fr")?.to_owned(),
                },
                synthetic: a.values.get("--synthetic").cloned(),
                system_protocol: 1,
            };
            let m = stage(&input).map_err(e)?;
            println!("{} (number {}) staged in {}", m.version, m.release_number.0, input.out_dir.display());
            Ok(())
        }
        ["publish", rest @ ..] => {
            let a = Args::parse(&words[1..1 + rest.len()])?;
            a.only(&["--repo", "--channel", "--keys", "--staged"])?;
            let channel = a.channel()?;
            let keys = KeySet::open(&a.path("--keys")?).map_err(e)?;
            let p = publish_release(&a.path("--repo")?, channel, &keys, &a.path("--staged")?, Expiry::for_channel(channel))
                .await
                .map_err(e)?;
            println!(
                "{} published on {} (targets v{}, timestamp v{})",
                p.manifest.version, channel.as_str(), p.targets_version, p.timestamp_version
            );
            Ok(())
        }
        ["refresh", rest @ ..] => {
            let a = Args::parse(&words[1..1 + rest.len()])?;
            a.only(&["--repo", "--channel", "--keys"])?;
            let channel = a.channel()?;
            let keys = KeySet::open(&a.path("--keys")?).map_err(e)?;
            let v = refresh(&a.path("--repo")?, channel, &keys, Expiry::for_channel(channel)).await.map_err(e)?;
            println!("timestamp v{v}");
            Ok(())
        }
        ["verify", rest @ ..] => {
            let a = Args::parse(&words[1..1 + rest.len()])?;
            a.only(&["--repo", "--channel", "--root"])?;
            let root = std::fs::read(a.path("--root")?).map_err(|x| x.to_string())?;
            let repo = load_dir(&a.path("--repo")?, a.channel()?, &root, false).await.map_err(e)?;
            for r in releases(&repo).await.map_err(e)? {
                println!("{}  number {}  {} bytes", r.manifest.version, r.manifest.release_number.0, r.manifest.space.download_bytes);
            }
            Ok(())
        }
        ["bundle", rest @ ..] => {
            let a = Args::parse(&words[1..1 + rest.len()])?;
            a.only(&["--repo", "--channel", "--version", "--out"])?;
            let version = ReleaseVersion::parse(a.get("--version")?).map_err(|x| x.to_string())?;
            ocinye_update_repository::offline::pack(&a.path("--repo")?, a.channel()?, &version, &a.path("--out")?)
                .await
                .map_err(e)
        }
        _ => Err("usage: ocinye-release keys|root|channel|stage|publish|refresh|verify|bundle … (see the source header)".into()),
    }
}

fn main() -> ExitCode {
    let words: Vec<String> = std::env::args().skip(1).collect();
    let rt = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("ocinye-release: {e}");
            return ExitCode::FAILURE;
        }
    };
    match rt.block_on(run(words)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("ocinye-release: {e}");
            ExitCode::FAILURE
        }
    }
}
