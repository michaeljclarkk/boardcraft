use boardcraft_board::BoardId;
use boardcraft_mobile_edit_host::Host;
use postgres::Config;
use uuid::Uuid;

fn main() -> Result<(), &'static str> {
    let board = std::env::args()
        .nth(1)
        .and_then(|s| Uuid::parse_str(&s).ok())
        .and_then(BoardId::new)
        .ok_or("expected a synthetic board UUID")?;
    let config: Config = std::env::var("RUSTDAY_TEST_DATABASE_URL")
        .map_err(|_| "missing disposable database URL")?
        .parse()
        .map_err(|_| "invalid database configuration")?;
    let host = Host::start(&config, board)?;
    println!(
        "Serving board {} on 127.0.0.1:{}; press Enter to stop.",
        board.as_uuid(),
        host.port
    );
    std::io::stdin()
        .read_line(&mut String::new())
        .map_err(|_| "stdin unavailable")?;
    Ok(())
}
