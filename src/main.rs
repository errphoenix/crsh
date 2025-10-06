use crsh_core::{Agent, AuthRequest, PreConnect, Remote, ServingClient};
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::io::stdout;
use std::io::Write;
use std::str::FromStr;
use std::time::Duration;
use tokio::time::{sleep_until, Instant};

#[derive(Debug)]
enum RunError {
    InitNoAddr,
    InitNoKey,
    InitInvalidKey,

    AuthConnectFailure,
}

impl Display for RunError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            RunError::InitNoAddr => write!(f, "no address provided"),
            RunError::InitNoKey => write!(f, "no key provided"),
            RunError::InitInvalidKey => write!(f, "invalid key provided (ensure int: [0,65535])"),
            RunError::AuthConnectFailure => {
                write!(f, "failed to connect or authenticate to master")
            }
        }
    }
}

impl Error for RunError {}

const NAMES: [&str; 8] = [
    "fra-cristoforo",
    "skibidi-toilet",
    "giacomo-leopardi",
    "renzo",
    "fracostein",
    "naranbaatar",
    "abdullah",
    "phoenix",
];

const USAGE: &str = r"
usage: crhs [OPTIONS] [ADDRESS:PORT/KEY]

OPTIONS:
  -h --help  - Print out this page

  --interval - Specify polling interval (in milliseconds)
";

fn arg_flag(arg: &str) -> bool {
    std::env::args().any(|a| a.eq(arg))
}

fn arg_var(arg: &str) -> Option<String> {
    let i = std::env::args().position(|a| a.eq(arg));
    if let Some(i) = i {
        std::env::args()
            .enumerate()
            .find(|(j, _)| i == j - 1)
            .map(|(_, s)| s)
    } else {
        None
    }
}

const VER_STR: &str = "v0.1.0-agent";

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
    }

    if arg_flag("--help") || arg_flag("-h") {
        let stdout = stdout();
        let mut lock = stdout.lock();
        writeln!(lock)?;
        writeln!(lock, "{USAGE}")?;
        return Ok(());
    }
    let interval = if let Some(interval) = arg_var("--interval") {
        Some(Duration::from_millis(interval.parse::<u64>()?))
    } else {
        None
    };

    let (master, key) = {
        let master_addr = std::env::args().next_back();
        if let Some(addr) = &master_addr {
            println!("Target master: {addr}");
            let (addr, key_str) = addr.rsplit_once("/").ok_or(RunError::InitNoKey)?;
            let key = key_str.parse::<u16>().map_err(|e| {
                eprintln!("{e}");
                RunError::InitInvalidKey
            })?;
            let master = Agent::new(Remote::from_str(addr)?);
            (master, key)
        } else {
            eprintln!("Error: no address provided");
            eprintln!("{USAGE}");
            return Err(RunError::InitNoAddr.into());
        }
    };
    let name = {
        let rng = rand::random_range(0..8);
        NAMES[rng]
    };
    let cached_token = std::fs::read("token").ok().and_then(|b| {
        let str = String::from_utf8(b).ok()?;
        if str.len() < 32 {
            println!("Malformed cached token string: length must be of 32. (A new one will be automatically generated once authenticated)");
            None
        } else {
            println!("Found valid cached token: {str}");
            Some(str)
        }
    });

    const RETRY_DELAY: Duration = Duration::from_secs(30);
    // AuthResult prints token when it's a success
    let (token, master) = {
        let mut agent = master;
        let conn = |agent: Agent<PreConnect>| {
            agent.try_connect(AuthRequest {
                client: name.to_string(),
                key,
                token: cached_token.clone(),
            })
        };
        let delay = async || sleep_until(Instant::now() + RETRY_DELAY).await;

        loop {
            match conn(agent).await {
                Ok(success) => break success,
                Err(failed_agent) => {
                    agent = failed_agent.reset();
                    eprintln!("Failed to authenticate agent.");
                    eprintln!(
                        "Will retry automatically in {} seconds...",
                        RETRY_DELAY.as_secs()
                    );
                    delay().await;
                }
            }
        }
    };

    {
        std::fs::write("token", token.to_string())?;
    }
    let mut client = ServingClient::new(master, token.to_string(), interval, name);
    client.run_recv().await;
    client.handle_reset().await;
    Ok(())
}
