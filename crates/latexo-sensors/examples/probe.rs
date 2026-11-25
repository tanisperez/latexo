//! Prints what this machine reports, twice one second apart: `cargo run -p latexo-sensors --example probe`.

fn main() {
    let Ok(mut sampler) = latexo_sensors::Sampler::new() else {
        eprintln!("could not read the system information");
        return;
    };
    println!("{:#?}", sampler.system_info());
    std::thread::sleep(std::time::Duration::from_secs(1));
    let mut sample = sampler.sample();
    sample
        .processes
        .sort_by(|a, b| b.cpu_percent.total_cmp(&a.cpu_percent));
    let processes = sample.processes.len();
    sample.processes.truncate(5);
    println!("{processes} processes");
    println!("{:#?}", sampler.capabilities(&sample));
    println!("{sample:#?}");
}
