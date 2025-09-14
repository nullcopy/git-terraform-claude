mod cli;
mod common;
mod duration_format;
mod error;
mod fit;
mod git_ops;
mod shift;
mod analyze;
mod timeline;
mod time_parser;
mod pour;

use clap::Parser;
use colored::*;
use std::process;

fn main() {
    let cli = cli::Cli::parse();
    
    let result: anyhow::Result<()> = match cli.command {
        cli::Commands::Shift {
            shift_amount,
            range,
            since,
            until,
            no_preview,
            allow_non_monotonic,
        } => {
            let config = shift::ShiftConfig::new(
                &shift_amount,
                range.as_deref(),
                since.as_deref(),
                until.as_deref(),
            );
            let options = shift::ShiftOptions::new(
                no_preview,
                allow_non_monotonic,
                cli.timeline_width,
                cli.verbose,
            );
            shift::execute_shift(config, options)
        }
        
        cli::Commands::Analyze {
            range,
            branch,
            since,
            until,
        } => {
            analyze::execute_analyze(
                range.as_deref(),
                branch.as_deref(),
                None,
                since.as_deref(),
                until.as_deref(),
                cli.timeline_width,
                cli.verbose,
            )
        }
        
        cli::Commands::Fit {
            target_range,
            range,
            since,
            until,
            skip_weekends,
            skip_weekdays,
            no_preview,
            allow_non_monotonic,
        } => {
            fit::execute_fit(
                &target_range,
                range.as_deref(),
                since.as_deref(),
                until.as_deref(),
                skip_weekends,
                skip_weekdays,
                no_preview,
                allow_non_monotonic,
                cli.timeline_width,
                cli.verbose,
            )
        }
        
        cli::Commands::Pour {
            count,
            message_prefix,
        } => {
            pour::execute_pour(
                count,
                &message_prefix,
                cli.timeline_width,
                cli.verbose,
            )
        }
    };
    
    if let Err(e) = result {
        eprintln!("{} {}", "Error:".red().bold(), e);
        process::exit(error::exit_code(&e));
    }
}