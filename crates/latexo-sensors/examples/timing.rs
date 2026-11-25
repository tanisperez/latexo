//! Times each refresh of sysinfo, to keep the sampler cheap:
//! `cargo run --release -p latexo-sensors --example timing`.

use std::time::Instant;

use sysinfo::{
    Components, CpuRefreshKind, DiskRefreshKind, Disks, Gpus, Networks, ProcessRefreshKind,
    ProcessesToUpdate, System, UpdateKind,
};

fn time(label: &str, runs: u32, mut f: impl FnMut()) {
    let start = Instant::now();
    for _ in 0..runs {
        f();
    }
    println!(
        "{label:<28} {:>8.2} ms",
        start.elapsed().as_secs_f64() * 1000.0 / f64::from(runs)
    );
}

fn main() {
    let Ok(mut system) = System::new() else {
        return;
    };
    let mut disks = Disks::new_with_refreshed_list().ok();
    let mut networks = Networks::new_with_refreshed_list().ok();
    let mut components = Components::new_with_refreshed_list().ok();
    let mut gpus = Gpus::new_with_refreshed_list().ok();
    let runs = 10;
    time("cpu", runs, || {
        system.refresh_cpu_specifics(CpuRefreshKind::everything())
    });
    time("memory", runs, || system.refresh_memory());
    let base = ProcessRefreshKind::nothing().with_cpu().with_memory();
    time("processes cpu+mem", runs, || {
        system.refresh_processes_specifics(ProcessesToUpdate::All, true, base);
    });
    time("+ disk usage", runs, || {
        system.refresh_processes_specifics(ProcessesToUpdate::All, true, base.with_disk_usage());
    });
    time("+ tasks", runs, || {
        system.refresh_processes_specifics(ProcessesToUpdate::All, true, base.with_tasks());
    });
    time("+ gpu usage", runs, || {
        system.refresh_processes_specifics(ProcessesToUpdate::All, true, base.with_gpu_usage());
    });
    time("+ user/exe once", runs, || {
        system.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            base.with_user(UpdateKind::OnlyIfNotSet)
                .with_exe(UpdateKind::OnlyIfNotSet),
        );
    });
    let all = ProcessRefreshKind::nothing()
        .with_cpu()
        .with_memory()
        .with_disk_usage()
        .with_tasks()
        .with_gpu_usage()
        .with_exe(UpdateKind::OnlyIfNotSet)
        .with_user(UpdateKind::OnlyIfNotSet);
    time("processes, everything", runs, || {
        system.refresh_processes_specifics(ProcessesToUpdate::All, true, all);
    });
    time("processes, no gpu", runs, || {
        system.refresh_processes_specifics(ProcessesToUpdate::All, true, all.without_gpu_usage());
    });
    time("processes, no tasks", runs, || {
        system.refresh_processes_specifics(ProcessesToUpdate::All, true, all.without_tasks());
    });
    time("disks", runs, || {
        if let Some(d) = disks.as_mut() {
            d.refresh_specifics(
                true,
                DiskRefreshKind::nothing().with_storage().with_io_usage(),
            );
        }
    });
    time("networks", runs, || {
        if let Some(n) = networks.as_mut() {
            n.refresh(true);
        }
    });
    time("components", runs, || {
        if let Some(c) = components.as_mut() {
            c.refresh(true);
        }
    });
    time("gpus", runs, || {
        if let Some(g) = gpus.as_mut() {
            g.refresh(true);
        }
    });
    let Ok(mut sampler) = latexo_sensors::Sampler::new() else {
        return;
    };
    time("whole sample", runs, || {
        let _ = sampler.sample();
    });
    time("sample without processes", runs, || {
        let _ = sampler.sample_with(false);
    });
}
