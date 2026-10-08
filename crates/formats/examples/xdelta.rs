//! `cargo run --release -p kaleido-formats --example xdelta -- original patch.xdelta sortie`
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [src, patch, out] = &args[..] else {
        eprintln!("usage : xdelta <original> <patch> <sortie>");
        std::process::exit(2);
    };
    let t = std::time::Instant::now();
    let target = kaleido_formats::vcdiff::apply(&std::fs::read(src).unwrap(), &std::fs::read(patch).unwrap()).unwrap();
    std::fs::write(out, &target).unwrap();
    println!("{} octets en {:?}", target.len(), t.elapsed());
}
