use std::io;
use std::io::{BufRead, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use clap::Args;

#[derive(Args, Clone)]
#[group(required = true, multiple = true)]
pub struct SearchLimit {
    /// The maximal depth for searches.
    #[arg(short, long)]
    pub depth: Option<u16>,

    /// The maximal node count for searches.
    #[arg(short, long)]
    pub nodes: Option<u64>,
}

impl SearchLimit {
    pub fn go_command(&self) -> String {
        let mut command = String::from("go");

        if let Some(depth) = self.depth {
            command.push_str(format!(" depth {}", depth).as_str());
        }

        if let Some(nodes) = self.nodes {
            command.push_str(format!(" nodes {}", nodes).as_str());
        }

        command.push('\n');
        command
    }
}

pub struct UciEngine {
    _proc: Child,
    stdin: ChildStdin,
    stdout: io::BufReader<ChildStdout>,
}

impl UciEngine {
    pub fn try_new(path: &str) -> io::Result<UciEngine> {
        let mut proc = Command::new(path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()?;

        let stdin = proc.stdin.take().unwrap();
        let stdout = io::BufReader::new(proc.stdout.take().unwrap());

        Ok(UciEngine {
            _proc: proc,
            stdin,
            stdout,
        })
    }

    pub fn write(&mut self, data: &[u8]) -> io::Result<()> {
        self.stdin.write_all(data)
    }

    pub fn read_line(&mut self) -> io::Result<String> {
        let mut buf = String::new();

        self.stdout.read_line(&mut buf)?;
        Ok(buf)
    }

    pub fn ready(&mut self) -> io::Result<()> {
        self.write(b"isready\n")?;

        loop {
            if let Some("readyok") = self.read_line()?.split(char::is_whitespace).next() {
                break;
            }
        }

        Ok(())
    }

    pub fn init_protocol(&mut self, config: &Vec<String>) -> io::Result<()> {
        self.write(b"uci\n")?;

        // TODO: additionally collect existing options in the engine and warn
        // in case of invalid/non-existent parameters in the config

        loop {
            if let Some("uciok") = self.read_line()?.split(char::is_whitespace).next() {
                break;
            }
        }

        for parameter in config {
            if let Some((name, value)) = parameter.split_once('=') {
                self.write(b"setoption name ")?;
                self.write(name.as_bytes())?;
                self.write(b" value ")?;
                self.write(value.as_bytes())?;
                self.write(b"\n")?;
                self.ready()?;
            }
        }

        Ok(())
    }

    pub fn setup_position(&mut self, fen: &str) -> io::Result<()> {
        self.write(b"ucinewgame\n")?;
        self.ready()?;
        self.write(b"position fen ")?;
        self.write(fen.as_bytes())?;
        self.write(b"\n")?;
        self.ready()
    }

    pub fn run_search(&mut self, limit: &SearchLimit) -> io::Result<i16> {
        self.write(limit.go_command().as_bytes())?;

        let mut score = None;

        loop {
            let line = self.read_line()?;
            let mut tokens = line.split(char::is_whitespace);

            match tokens.next() {
                Some("info") => (),
                Some("bestmove") => break,
                _ => return Err(io::Error::from(io::ErrorKind::InvalidInput)),
            }

            while let Some(token) = tokens.next() {
                match token {
                    "score" => match (tokens.next(), tokens.next()) {
                        (Some("cp"), Some(v)) => {
                            score = Some(
                                v.parse()
                                    .map_err(|_| io::Error::from(io::ErrorKind::InvalidInput))?,
                            );
                        }
                        (Some("mate"), Some(v)) => {
                            let mate = v
                                .parse::<i16>()
                                .map_err(|_| io::Error::from(io::ErrorKind::InvalidInput))?;
                            score = Some(if mate <= 0 {
                                mate - 32000
                            } else {
                                32000 - mate
                            });
                        }
                        _ => return Err(io::Error::from(io::ErrorKind::InvalidInput)),
                    },
                    "wdl" => {
                        let _ = tokens.nth(2);
                    }
                    "upperbound" => (),
                    "lowerbound" => (),
                    "pv" => break,
                    _ => {
                        let _ = tokens.next();
                    }
                }
            }
        }

        score.ok_or(io::Error::from(io::ErrorKind::UnexpectedEof))
    }
}
