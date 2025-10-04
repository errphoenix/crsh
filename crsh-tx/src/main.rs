use crsh_core::{Command, MasterEndpoint, Remote, SubmitRequest};
use std::error::Error;
use std::fs;
use std::io::Write;
use std::io::{stdin, stdout};
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

    let mut input = String::new();
    let stdin = stdin();

    let mut endpoint: Option<MasterEndpoint> = None;

    loop {
        stdin.read_line(&mut input)?;
        input = input.trim().to_string();
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
            "quit" => break,
            _ => print_help()?,
        }
        input.clear();
    }

    Ok(())
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
    writeln!(lock, "   [ADDRESS:PORT]")?;

    writeln!(lock)?;
    writeln!(lock, "MISCELLANEOUS")?;
    writeln!(lock, "   help  Show this list of commands")?;
    writeln!(lock, "   quit  Terminate application")?;
    writeln!(lock)?;
    writeln!(lock)?;

    Ok(())
}
