//! `gateway --bundles DIR --data DIR [--repo CLONE] [--plugin-host "CMD ARGS"] [--github URL]
//!  [--github-repo OWNER/REPO] [--pull N] [--prepare "CMD ARGS"] [--port 9260]`
use gateway::{http, Config, Gateway};
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::Arc;

fn main() {
    let mut cfg = Config { github: "http://127.0.0.1:9261".into(), github_repo: "fork/junction26-messagepack-csharp".into(), ..Default::default() };
    cfg.bundles = PathBuf::from("work/bundles");
    cfg.data = PathBuf::from("work/gateway");
    let mut port = 9260u16;
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < args.len() {
        let v = args.get(i + 1).cloned().unwrap_or_else(|| {
            eprintln!("missing value for {}", args[i]);
            std::process::exit(2)
        });
        match args[i].as_str() {
            "--bundles" => cfg.bundles = v.into(),
            "--data" => cfg.data = v.into(),
            "--repo" => cfg.repo = Some(v.into()),
            "--plugin-host" => cfg.plugin_host = v.split_whitespace().map(String::from).collect(),
            "--prepare" => cfg.prepare = v.split_whitespace().map(String::from).collect(),
            "--plugins" => cfg.plugins.push(v.into()),
            "--trees" => cfg.trees = Some(v),
            "--web" => cfg.web = Some(v.into()),
            "--plugin-timeout" => cfg.plugin_timeout = Some(std::time::Duration::from_secs(v.parse().expect("seconds"))),
            "--github" => cfg.github = v,
            "--github-repo" => cfg.github_repo = v,
            "--pull" => cfg.pull_request = v.parse().ok(),
            "--port" => port = v.parse().expect("port"),
            a => {
                eprintln!("unknown argument {a}");
                std::process::exit(2)
            }
        }
        i += 2;
    }
    // Older start scripts name the plugins folder only inside `--plugin-host "... --plugins DIR"`.
    if cfg.plugins.is_empty() {
        if let Some(p) = cfg.plugin_host.iter().position(|a| a == "--plugins") {
            if let Some(d) = cfg.plugin_host.get(p + 1) {
                cfg.plugins.push(d.into());
            }
        }
    }
    let l = TcpListener::bind(("127.0.0.1", port)).unwrap_or_else(|e| {
        eprintln!("bind 127.0.0.1:{port}: {e}");
        std::process::exit(1)
    });
    eprintln!("gateway on http://127.0.0.1:{port}");
    let g = Arc::new(Gateway::new(cfg));
    http::serve(l, move |r| g.handle(r));
}
