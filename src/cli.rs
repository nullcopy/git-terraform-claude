use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "git-terraform",
    about = "Adjust and analyze commit timestamps in a Git repository",
    long_about = "Manipulate and visualize the temporal structure of commits in a repository.\n\
                  When shifting commits, GPG signing is automatically disabled since GPG\n\
                  signatures contain immutable timestamps.",
    version,
    author
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
    
    #[arg(short = 'w', long, default_value = "80", global = true, 
          help = "Width of the timeline visualization")]
    pub timeline_width: usize,
    
    #[arg(short, long, global = true, help = "Show detailed information during processing")]
    pub verbose: bool,
}

#[derive(Subcommand)]
pub enum Commands {
    #[command(about = "Shift commit timestamps forward or backward")]
    Shift {
        #[arg(
            help = "Time shift in format: [+|-]<value><unit>\n\
                      Examples: +3d, -2h, +1y6m, -30m\n\
                      Units: s(econds), m(inutes), h(ours), d(ays),\n\
                             w(eeks), M(onths), y(ears)",
            allow_hyphen_values = true
        )]
        shift_amount: String,
        
        #[arg(short, long, help = "Specify commits to shift using Git revision syntax\n\
                                   Examples:\n\
                                     HEAD~10..HEAD  (last 10 commits)\n\
                                     main..HEAD     (commits not in main)\n\
                                     @{u}..HEAD     (commits not in upstream)")]
        range: Option<String>,
        
        #[arg(long, help = "Only shift commits after this date")]
        since: Option<String>,
        
        #[arg(long, help = "Only shift commits before this date")]
        until: Option<String>,
        
        #[arg(long, help = "Skip the preview timeline and apply changes immediately")]
        no_preview: bool,
        
        #[arg(long, help = "Proceed even if the operation creates a non-monotonic timeline")]
        allow_non_monotonic: bool,
    },
    
    #[command(about = "Display timeline visualization without changes")]
    Analyze {
        #[arg(short, long, help = "Analyze specific commits using Git revision syntax")]
        range: Option<String>,
        
        #[arg(short, long, help = "Analyze commits in specified branch (without checking out)")]
        branch: Option<String>,
        
        #[arg(long, help = "Include commits after this date in analysis")]
        since: Option<String>,
        
        #[arg(long, help = "Include commits before this date in analysis")]
        until: Option<String>,
    },
    
    #[command(about = "Fit commits into a specified time range")]
    Fit {
        #[arg(
            help = "Target time range in format: YYYY-MM-DD..YYYY-MM-DD\n\
                    Can also include times: YYYY-MM-DD HH:MM:SS..YYYY-MM-DD HH:MM:SS\n\
                    Examples:\n\
                      2024-01-01..2024-12-31  (fit into year 2024)\n\
                      2024-06-01..2024-06-30  (fit into June 2024)\n\
                      \"7 days ago..now\"       (fit into last week)"
        )]
        target_range: String,
        
        #[arg(short, long, help = "Specify commits to fit using Git revision syntax")]
        range: Option<String>,
        
        #[arg(long, help = "Only fit commits after this date")]
        since: Option<String>,
        
        #[arg(long, help = "Only fit commits before this date")]
        until: Option<String>,
        
        #[arg(long, help = "Skip weekends when distributing commits")]
        skip_weekends: bool,
        
        #[arg(long, help = "Skip weekdays when distributing commits")]
        skip_weekdays: bool,
        
        #[arg(long, help = "Skip the preview timeline and apply changes immediately")]
        no_preview: bool,
        
        #[arg(long, help = "Proceed even if the operation creates a non-monotonic timeline")]
        allow_non_monotonic: bool,
    },
    
    #[command(about = "Pour test commits onto today for timeline testing")]
    Pour {
        #[arg(
            help = "Number of test commits to create"
        )]
        count: usize,
        
        #[arg(long, help = "Custom prefix for commit messages", default_value = "")]
        message_prefix: String,
    },
}

