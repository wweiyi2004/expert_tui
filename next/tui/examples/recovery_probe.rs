//! Isolated acceptance probe: the runner kills its first host after "ready".
use expert_tui::{
    launcher::LauncherOptions,
    source::{BridgeSource, DataSource, SourceEvent},
};
use std::time::{Duration, Instant};

fn main() -> anyhow::Result<()> {
    let profile = std::env::var("EXPERT_HOST_DATA_DIR")?;
    anyhow::ensure!(
        profile.contains("expert-host-recovery-"),
        "Isolated profile required"
    );
    let mut source = BridgeSource::launch(LauncherOptions {
        auto_start: true,
        ..Default::default()
    });
    let start = Instant::now();
    let mut connections = 0;
    let mut announced = false;
    loop {
        for event in source.poll() {
            match event {
                SourceEvent::Connection {
                    connected: true, ..
                } => connections += 1,
                SourceEvent::Snapshot(_) if !announced => {
                    println!("ready");
                    announced = true;
                }
                SourceEvent::Snapshot(_) if connections >= 2 => {
                    println!("recovered");
                    source.close();
                    return Ok(());
                }
                _ => {}
            }
        }
        anyhow::ensure!(
            start.elapsed() < Duration::from_secs(45),
            "Recovery timed out"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}
