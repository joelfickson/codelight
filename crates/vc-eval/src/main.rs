use anyhow::Result;
use clap::Parser;
use vc_eval::{Case, evaluate};

#[derive(Parser)]
struct Cli {
    #[arg(long, value_enum)]
    case: Option<Case>,
    #[arg(
        long,
        help = "Use the configured Gateway model instead of scripted responses"
    )]
    live: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let cases = cli
        .case
        .map(|case| vec![case])
        .unwrap_or_else(|| vec![Case::Boundary, Case::Refactor, Case::TestRecovery]);
    let mut passed = true;
    for case in cases {
        let report = evaluate(case, cli.live).await?;
        passed &= report.passed;
        println!("{}", serde_json::to_string(&report)?);
    }
    anyhow::ensure!(passed, "one or more coding evaluations failed");
    Ok(())
}
