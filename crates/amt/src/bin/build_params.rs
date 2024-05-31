use amt::{AMTParams, AMTVerifyParams};
use anyhow::{bail, Result};
use tracing::Level;
use tracing_subscriber::fmt::format::FmtSpan;

fn parse_param() -> Result<(usize, usize, usize, usize)> {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 4 {
        bail!(
            "Usage: {} <amt-depth> <verify-depth> <coset-index> <high-depth>",
            args[0]
        );
    }

    Ok((args[1].parse()?, args[2].parse()?, args[3].parse()?, args[4].parse()?))
}

fn main() {
    let (expected_depth, verify_depth, coset, expected_high_depth) = match parse_param() {
        Ok(x) => x,
        Err(e) => {
            eprintln!("Cannot parse input: {:?}", e);
            std::process::exit(1);
        }
    };

    tracing_subscriber::fmt()
        .with_max_level(Level::DEBUG)
        .with_span_events(FmtSpan::CLOSE)
        .with_target(false)
        .init();

    AMTParams::from_dir_mont("./pp", expected_depth, true, coset, expected_high_depth);
    AMTVerifyParams::from_dir_mont("./pp", expected_depth, verify_depth, coset, expected_high_depth);
}
