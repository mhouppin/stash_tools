use clap::Parser;

use std::fs::File;
use std::io::prelude::*;
use std::io::{stdout, BufReader};
use std::thread;
use std::time::Instant;

pub mod engine;
pub mod task_queue;

use crate::engine::SearchLimit;
use crate::task_queue::{TaskClient, TaskWorker};

/// This tool allows for scoring chess positions coming from a text-based
/// dataset file.
///
/// The expected input format of the dataset is <FEN WDL>, with FEN being a
/// chess position written in Forsyth-Edwards Notation, and WDL being a decimal
/// number representing the game result from White's point of view (1.0 for a
/// White win, 0.0 for a Black win, and 0.5 for draw).
///
/// The output format is <FEN WDL EVAL>, with EVAL being the returned search
/// score from the engine, from the side to move's point of view.
#[derive(Parser)]
#[command(author, version, about, long_about, verbatim_doc_comment)]
struct Cli {
    /// The path of the engine to use for scoring
    #[arg(short, long)]
    engine_path: String,

    /// An UCI option which should be passed to the engine at startup.
    /// You can use this flag as many times as you need.
    #[arg(short, long)]
    config: Vec<String>,

    /// The file containing the positions to score.
    #[arg(short, long)]
    input_file: String,

    /// The output file for scored positions. Note that it will overwrite any
    /// already existing file with the given name.
    #[arg(short, long)]
    output_file: String,

    /// The number of threads/engine instances to use for scoring.
    #[arg(short, long, default_value_t = 1)]
    threads: usize,

    #[command(flatten)]
    limit: SearchLimit,

    /// How frequently should progress be reported, in terms of scored positions.
    #[arg(short, long, default_value_t = 1000)]
    report_every: usize,
}

fn main() -> std::io::Result<()> {
    let cli = Cli::parse();
    let mut client = TaskClient::new();
    let ifile = File::open(cli.input_file.as_str())?;
    let mut ofile = File::create(cli.output_file.as_str())?;
    let mut reader = BufReader::new(ifile);
    let mut thread_list = Vec::new();

    let mut queries: usize = 0;
    let mut responses: usize = 0;
    let start = Instant::now();

    for _ in 0..cli.threads {
        let mut worker = TaskWorker::new(client.queue_ref(), cli.engine_path.as_str(), &cli.config);
        let limit = cli.limit.clone();

        thread_list.push(thread::spawn(move || {
            while let Some(workload) = worker.query_workload() {
                let last_space_idx = workload.rfind(' ').unwrap();
                let (fen, value) = workload.split_at(last_space_idx);
                let value = value.trim().parse::<f32>().unwrap();

                worker.engine_mut().setup_position(fen).unwrap();

                let score = worker.engine_mut().run_search(&limit).unwrap();
                let scored_fen = format!("{} {} {}\n", fen, value, score);
                worker.fill_response(scored_fen);
            }

            worker.remove_worker();
        }));
    }

    loop {
        let mut buf = String::new();
        let read_size = reader.read_line(&mut buf)?;

        if read_size == 0 {
            break;
        }

        client.add_workload(buf);
        queries += 1;

        if let Some(scored_fen) = client.query_response(false) {
            ofile.write_all(scored_fen.as_bytes())?;
            responses += 1;

            if responses % cli.report_every == 0 {
                let elapsed = start.elapsed().as_secs_f32();
                let ett = elapsed / (responses as f32) * (queries as f32);
                let eta = ett - elapsed;

                print!(
                    "\r{}/{} queries done, {:.3} seconds elapsed, ETA {:.3} seconds    ",
                    responses, queries, elapsed, eta
                );
                stdout().flush()?;
            }
        }
    }

    client.stop_workload();

    while let Some(scored_fen) = client.query_response(true) {
        ofile.write_all(scored_fen.as_bytes())?;
        responses += 1;

        if responses % cli.report_every == 0 {
            let elapsed = start.elapsed().as_secs_f32();
            let ett = elapsed / (responses as f32) * (queries as f32);
            let eta = ett - elapsed;

            print!(
                "\r{}/{} queries done, {:.3} seconds elapsed, ETA {:.3} seconds    ",
                responses, queries, elapsed, eta
            );
            stdout().flush()?;
        }
    }

    println!();

    for thread in thread_list {
        thread.join().unwrap();
    }

    Ok(())
}
