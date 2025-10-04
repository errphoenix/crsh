use crsh_core::{Command, MasterEndpoint, Remote, SubmitRequest};
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;
use std::error::Error;
use std::fs;
use std::io::stdout;
use std::io::{stderr, Write};
use std::str::FromStr;

const VER_STR: &str = "v0.1.0-tx";

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    {
        let stdout = stdout();
        let mut lock = stdout.lock();
        writeln!(
            lock,
            "Centralised Remote Shell - tiny remote shell execution protocol"
        )?;
        writeln!(lock, "Version: {VER_STR}")?;
        writeln!(lock, "Author: HerrPhoenix")?;
        writeln!(lock)?;
        writeln!(lock, "Type 'help' for a list of commands.")?;
    }

    let mut rl = DefaultEditor::new()?;
    let mut endpoint: Option<MasterEndpoint> = None;
    loop {
        let input = rl.readline(">> ");
        match input {
            Ok(input) => {
                rl.add_history_entry(input.as_str())?;
                if input.is_empty() {
                    continue;
                }

                let label: Vec<&str> = input.splitn(2, " ").collect();
                match label[0] {
                    "bind" => {
                        let args: Vec<&str> = input.split_whitespace().collect();
                        if args.len() < 2 {
                            print_help()?;
                        } else {
                            let addr = args[1];
                            let remote = Remote::from_str(addr)?;
                            endpoint = conn_endpoint(remote).await;
                        }
                    }
                    "put" => {
                        if let Some(session) = &endpoint {
                            fs::write("session", session.to_string())?;
                            println!(
                                "Wrote current session {session} to memory. You can use 'pop' to load it from memory."
                            )
                        } else {
                            eprintln!(
                                "You are currently not bound to any session, use 'bind' to connect to one."
                            )
                        }
                    }
                    "pop" => {
                        let mem = String::from_utf8(fs::read("session")?)?;
                        if mem.is_empty() {
                            eprintln!(
                                "No session stored in memory. You must first store a valid session using 'put'."
                            )
                        } else {
                            fs::write("session", "")?;
                            match MasterEndpoint::parse(&mem) {
                                Ok(ep) => endpoint = conn_endpoint(ep.0).await,
                                Err(e) => eprintln!("Failed to parse session from memory:\n{e}"),
                            };
                        }
                    }
                    "cmd" => {
                        if let Some(session) = &endpoint {
                            let args: Vec<&str> = input.split_whitespace().skip(1).collect();
                            if args.len() > 2 && args[0].eq("--target") {
                                let token = args[1].to_string();
                                let args: Vec<&str> = args.iter().skip(2).copied().collect();
                                submit(session, Some(token), args).await;
                            } else {
                                submit(session, None, args).await;
                            }
                        } else {
                            eprintln!(
                                "You are currently not bound to any session, use 'bind' to connect to one."
                            )
                        }
                    }
                    "query" => {
                        let args: Vec<&str> = input.split_whitespace().skip(1).collect();
                        let (n_i, count): (Option<usize>, Option<usize>) = {
                            let i = args.iter().position(|&a| a.eq("-N"));
                            (
                                i.map(|i| i + 1),
                                i.and_then(|i| args.get(i + 1).and_then(|s| s.parse().ok())),
                            )
                        };

                        let fits = n_i.is_some_and(|i| args.len() > i + 1) || n_i.is_none();
                        let endpoint = if fits && let Some(addr) = args.last() {
                            let remote = Remote::from_str(addr);
                            if let Err(e) = &remote {
                                eprintln!("Invalid address provided '{addr}': {e}");
                                None
                            } else {
                                let remote = remote.unwrap();
                                Some(&MasterEndpoint::new(remote))
                            }
                        } else {
                            endpoint.as_ref()
                        };
                        if let Some(endpoint) = endpoint {
                            query(endpoint, count).await;
                        } else {
                            eprintln!(
                                "Invalid endpoint. Either the endpoint provided is not correct, or you are not bound to any."
                            );
                        }
                    }
                    "quit" => break,
                    _ => print_help()?,
                }
            }
            Err(ReadlineError::Interrupted) | Err(ReadlineError::Eof) => {
                println!();
                break;
            }
            Err(err) => {
                eprintln!("Readline error: {err}");
                break;
            }
        }
    }
    Ok(())
}

const DEFAULT_QUERY_COUNT: usize = 10;

async fn query(endpoint: &MasterEndpoint, count: Option<usize>) {
    let count = count.unwrap_or(DEFAULT_QUERY_COUNT);
    if endpoint.0.ping().await.is_ok() {
        match endpoint.query().await {
            Ok(hist) => {
                let diff = ((hist.0.len() - count) as i32).max(0) as usize;

                let mut err = stderr().lock();
                let mut out = stdout().lock();

                hist.0
                    .iter()
                    .skip(diff)
                    .for_each(|line| match line.out_type {
                        crsh_core::OutType::Err => writeln!(err, "{line}").unwrap(),
                        crsh_core::OutType::Out => writeln!(out, "{line}").unwrap(),
                    })
            }
            Err(e) => {
                eprintln!("Failed to query history: {e:?}")
            }
        }
    }
}

async fn submit(endpoint: &MasterEndpoint, target: Option<String>, mut args: Vec<&str>) {
    if args.is_empty() {
        eprintln!("Cannot send empty commands.");
        return;
    }
    if endpoint.0.ping().await.is_ok() {
        let cmd = {
            let mut cmd_str = String::new();
            args.drain(..).for_each(|s| {
                cmd_str.push_str(s);
                cmd_str.push(' ');
            });
            Command(cmd_str)
        };

        let req = if let Some(token) = target {
            SubmitRequest::Single { token, cmd }
        } else {
            SubmitRequest::Broadcast { cmd }
        };
        if let Err(e) = endpoint.submit(req).await {
            eprintln!("Failed to send command: {e:?}");
        }
    }
}

async fn conn_endpoint(remote: Remote) -> Option<MasterEndpoint> {
    let mut res: Option<MasterEndpoint> = None;
    match remote.ping().await {
        Ok(ms) => {
            println!("Connected to master endpoint {remote} in {ms}ms.");
            res = Some(MasterEndpoint::new(remote));
            println!(
                "Successfully bound session to router. You can use 'put' to store it in memory."
            );
        }
        Err(e) => eprintln!("Failed to connect to master endpoint {remote}:\n{e}"),
    }
    res
}

fn print_help() -> Result<(), Box<dyn Error>> {
    let stdout = stdout();
    let mut lock = stdout.lock();
    writeln!(
        lock,
        "Centralised Remote Shell - tiny remote shell execution protocol"
    )?;
    writeln!(lock, "Version: {VER_STR}")?;
    writeln!(lock, "Author: HerrPhoenix")?;

    writeln!(lock)?;
    writeln!(lock, "SESSION CONTROL")?;
    writeln!(lock, "   bind  Bind session to a CRSH router")?;
    writeln!(lock, "   ADDRESS:PORT")?;
    writeln!(lock)?;
    writeln!(lock, "   put   Store current CRSH router session to memory")?;
    writeln!(
        lock,
        "   pop   Load and bind last CRSH router session in memory"
    )?;

    writeln!(lock)?;
    writeln!(lock, "CORE FUNCTIONS")?;
    writeln!(lock, "   cmd   Queue a command to the CRSH router")?;
    writeln!(lock, "   [--target TOKEN] COMMAND...")?;
    writeln!(lock)?;
    writeln!(lock, "   query Query CRSH router out + err history")?;
    writeln!(
        lock,
        "   [-N {DEFAULT_QUERY_COUNT} [1,1024]] [ADDRESS:PORT]"
    )?;

    writeln!(lock)?;
    writeln!(lock, "MISCELLANEOUS")?;
    writeln!(lock, "   help  Show this list of commands")?;
    writeln!(lock, "   quit  Terminate application")?;
    writeln!(lock)?;
    writeln!(lock)?;

    Ok(())
}
